use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::ops::Deref;
use std::sync::Arc;

use specs::Component;
use specs::Entities;
use specs::Entity;
use specs::HashMapStorage;
use specs::Join;
use specs::LazyUpdate;
use specs::Read;
use specs::ReadStorage;
use specs::World;
use specs::WorldExt;
use specs::WriteStorage;
use tracing::trace;
use tracing::warn;

use crate::v2::property_value;
use crate::v2::ActionBuffer;
use crate::v2::Property;
use crate::v2::WorldWrapper;
use crate::Error;
use crate::Identifier;

use super::DispatchSignature;
use super::compiler::Object;
use super::BuildLog;
use super::DispatchRef;
use super::Dispatch;
use super::Properties;
use super::Visitor;

/// Struct for implementing a config framework,
///
#[derive(Component, Debug, Clone)]
#[storage(HashMapStorage)]
pub struct Framework {
    /// Original build entity,
    ///
    entity: Entity,
    /// Roots to filter by,
    ///
    roots: BTreeSet<String>,
    /// Registered identifier patterns to query for,
    ///
    patterns: Vec<String>,
    /// Config identifier patterns to query for,
    ///
    config_patterns: Vec<(String, ConfigPattern)>,
    /// Config properties,
    ///
    config_properties: BTreeMap<String, Arc<Properties>>,
    /// Queue for configuring extensions,
    ///
    config_queue: VecDeque<Identifier>,
}

impl Framework {
    /// Returns a new empty framework,
    ///
    pub const fn new(entity: Entity) -> Self {
        Self {
            entity,
            roots: BTreeSet::new(),
            patterns: vec![],
            config_patterns: vec![],
            config_properties: BTreeMap::new(),
            config_queue: VecDeque::new(),
        }
    }

    /// Returns the root interpolation patterns,
    ///
    fn patterns(&self) -> impl Iterator<Item = &String> {
        self.patterns.iter()
    }

    /// Returns the config interpolation patterns,
    ///
    fn config_patterns(&self) -> impl Iterator<Item = &(String, ConfigPattern)> {
        self.config_patterns.iter()
    }

    /// Checks an identifier w/ root interpolation patterns,
    ///
    fn check(&self, other: &Identifier) -> Option<(String, ConfigPattern)> {
        for map in self.patterns().filter_map(|p| other.interpolate(p)) {
            let name = &map["name"].to_lowercase();
            let property = &map["property"];
            // If the name is the same as the property, it's implied that its referencing itself
            if name == property {
                return Some((
                    format!("{:#}", other),
                    ConfigPattern::NameInput(format!("#root#.{}.(?input)", name)),
                ));
            } else {
                // Otherwise, the pattern is {name}.{pattern}.(input)
                return Some((
                    format!("{:#}", other),
                    ConfigPattern::NamePropertyInput(format!("#root#.{}.{}.(?input)", name, property)),
                ));
            }
        }

        None
    }

    /// Analyzes and extracts actions from properties,
    /// 
    pub fn analyze(&self, properties: &Properties) -> Result<ActionBuffer, Error> {
        let owner = format!("{:#}", properties.owner());
        trace!("Analyzing  -- {}", owner);
        trace!("           -- {}", properties);
        let subject = properties.owner().subject().to_string();
        let mut owner = owner.parse::<Identifier>()?;
        let mut found = vec![];

        let read_only = Arc::new(properties.clone());

        // Check identifier of owner,
        for (pattern, ext_config_pattern) in self.config_patterns() {
            trace!("Pattern  -- {}/{:?}", pattern, ext_config_pattern);
            match ext_config_pattern {
                ConfigPattern::NameInput(config_pattern) => {
                    if let Some(map) = owner.interpolate(config_pattern) {
                        let input = map.get("input").unwrap_or(&subject);

                        let properties = read_only
                            .clone()
                            .branch(pattern, Some(property_value(input)))?;

                        found.push((
                            Property::Properties(properties.into()),
                            owner.clone(),
                            pattern,
                            config_pattern,
                            input.to_string(),
                        ));

                        trace!("Truncating --> {:?}", owner);
                        owner = owner.truncate(1)?;
                        owner.add_tag("root");
                        trace!("Truncated  --> {:#}", owner);
                        break;
                    } else {
                        trace!("NameInput skipped -- {:#}", owner);
                    }
                }
                ConfigPattern::NamePropertyInput(config_pattern) => {
                    if let Some(map) = owner.interpolate(config_pattern) {
                        if let Some(input) = map.get("input") {
                            let properties = read_only
                                .clone()
                                .branch(pattern, Some(property_value(input)))?;
                            
                            found.push((
                                Property::Properties(properties.into()),
                                owner.clone(),
                                pattern,
                                config_pattern,
                                map["input"].clone(),
                            ));
                        } else {

                        }
                    }
                }
            }
        }

        // Otherwise, check properties
        for (name, prop) in properties.iter_properties() {
            let ident = owner.branch(name)?;

            for (pattern, config_pattern) in self.config_patterns() {
                match config_pattern {
                    ConfigPattern::NamePropertyInput(config_pattern) => {
                        // Promote properties into an extension
                        if let Some(messages) = prop.as_symbol_vec() {
                            for message in messages.iter() {
                                let _ident = ident.branch(message)?;
                                trace!("Pattern  -- {:#} -- {}/{:?}", _ident, pattern, config_pattern);
                                if let Some(map) = _ident.interpolate(config_pattern) {
                                    let mut prop = prop.clone();
                                    trace!("-- {:?}", map);
                                    // If this config has additional properties then extend the existing property and merge the additional properties
                                    if let Some(properties) = self.config_properties.get(pattern) {
                                        trace!("existing config -- {}, {name}", properties);
                                        let mut properties = properties.deref().clone();
                                        properties[name] = property_value(message);
                                        prop = Property::Properties(Arc::new(properties));
                                    }

                                    found.push((
                                        prop.clone(),
                                        ident.clone(),
                                        pattern,
                                        config_pattern,
                                        map["input"].clone(),
                                    ));
                                }
                            }
                        }
                    }
                    _ => continue,
                }
            }
        }

        let mut action_buffer = ActionBuffer::new();
        for (property, ident, pattern, config_pattern, input) in found {
            if let Some(properties) = property.as_properties() {
                trace!(
                    "Found config \n{} {:#} -- {pattern} --> {config_pattern} --> {input}",
                    properties,
                    ident
                );
                let config_ident = pattern.parse::<Identifier>()?;
                if let Some(properties) = self.config_properties.get(&format!("{:#}", properties.owner())) {
                    for (name, prop) in properties.iter_properties() {
                        let config_ident = config_ident.branch(name)?;
                        action_buffer.push_config(&config_ident, &prop);
                    }
                }

                action_buffer.push_config(&config_ident, &property);
            } else {
                trace!(
                    "(Skipping) Found config {:?} {:#} -- {pattern} --> {config_pattern} --> {input}",
                    property,
                    ident
                );
            }
        }

        Ok(action_buffer)
    }
}

/// Enumeration of config patterns,
///
#[derive(Debug, Clone)]
enum ConfigPattern {
    /// Pattern of {name}.(input)
    ///
    NameInput(String),
    /// Pattern of {name}.{property}.(input)
    ///
    NamePropertyInput(String),
}

impl Visitor for Framework {
    fn visit_object(&mut self, object: &Object) {
        // Skip blocks,
        if object.is_block() {
            return;
        }

        // Scan roots for additional rules to configure
        if object
            .as_root()
            .map(|r| {
                self.visit_root(r);
            })
            .is_some()
        {
            // If a root was visited then skip visiting properties,
            return;
        }

        self.visit_properties(object.properties());
    }

    fn visit_properties(&mut self, properties: &Properties) {
        if properties.len() == 0 {
            return;
        }

        let matches = DispatchSignature::get_match(properties.owner());
        if matches.iter().any(|m| {
            match m {
                DispatchSignature::ConfigRoot { .. } |
                DispatchSignature::ConfigRootExt { .. } |
                DispatchSignature::ConfigExtendedProperty { .. } |
                DispatchSignature::ExtendedProperty { .. } => false,
                _ => true
             }
        }) || matches.is_empty() {
            trace!("Skipping {:?}, {:#}", matches, properties.owner());
            return;
        } else {
            trace!("Accepting {:?}, {:#}", matches, properties.owner());
        }

        let key = format!("{:#}", properties.owner());

        if !self.config_properties.contains_key(&key) {
            trace!("Adding -- {}", key);

            self.config_properties
                .insert(key, Arc::new(properties.clone()));
        }
    }

    fn visit_extension(&mut self, identifier: &Identifier) {
        // This means this extension needs to be configured
        if let Some(map) = identifier
            .commit()
            .unwrap()
            .interpolate("#block#.#root#.(name).(ext_root).(ext_name)")
        {
            trace!("Queueing config -- {:?}", map);
            if self.roots.contains(&map["ext_root"]) {
                self.config_queue.push_back(identifier.clone());
                return;
            }
        }

        // Only roots defined in the root block can apply config to the framework
        trace!("Scanning for new config for framework -- {:#}", identifier);

        if let Some(map) = identifier
            .commit()
            .unwrap()
            .interpolate("!#block#.#root#.(root).(ext).(?property)")
        {
            trace!("!!!!!!!!!!!!!!!! -----> {:?}", map);
            if let Some(pattern) = map.get("property").and_then(|_| self.check(identifier)) {
                trace!("Adding new config pattern      -- {:?}", pattern);
                self.config_patterns.push(pattern);
            } else {
                let root = &map["root"];
                let extension = &map["ext"];
                let root = format!("{root}").trim_matches('.').to_string();
                let pattern = format!("!#block#.#root#.{root}.(name).{extension}.(property)");
                if self.roots.insert(root.to_string()) {
                    trace!("Adding new root                -- {root}");
                }
                trace!("Adding new root pattern        -- {root}/{pattern}");
                self.patterns.push(pattern);
            }
            trace!("!!!!!!!!!!!!!!!");
            return;
        } else {
            warn!("Skipping, Didn't interpolate, {:#}", identifier);
            trace!(
                "-- Framework only recognizes the pattern !#block#.#root#.(root).(ext).(?property)"
            );
        }
    }
}

impl Dispatch for Framework {
    fn dispatch<'a>(
        &self,
        build_ref: DispatchRef<'a, Properties>,
    ) -> Result<DispatchRef<'a, Properties>, Error> {
        build_ref
            .map(|r| {
                self.analyze(r)
            })
            .result()
    }
}

/// Configures the framework for each build,
///
pub fn configure(world: &mut World) {
    world.exec(
        |(lazy_update, entities, logs, mut frameworks): (
            Read<LazyUpdate>,
            Entities,
            ReadStorage<BuildLog>,
            WriteStorage<Framework>,
        )| {
            for (e, log, mut framework) in (&entities, &logs, frameworks.drain()).join() {
                // Skip the original framework build,
                if e == framework.entity {
                    continue;
                }

                while let Some(config) = framework.config_queue.pop_front() {
                    trace!("Lazily searching build {:?} for {:#}", e, config);
                    let log = log.clone();
                    let config = config.clone();
                    let framework = framework.clone();
                    lazy_update.exec_mut(move |world| {
                        let mut wrapper = WorldWrapper::from(world);
                        log.find_ref(config, &mut wrapper).map(|r| {
                            framework.dispatch(r).ok().map(|_| {});
                        });
                    });
                }
            }
        },
    );

    world.maintain();
}
