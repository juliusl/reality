use specs::Component;
use specs::Entity;
use specs::VecStorage;
use tracing::trace;
use tracing::warn;

use crate::Error;
use crate::Identifier;
use crate::Value;
use crate::v2::EntityVisitor;

use super::DispatchSignature;
use super::Property;
use super::Visitor;

/// Enumeration of attribute actions that apply during the transient phase of the attribute's lifecycle,
///
#[derive(Clone, Debug)]
pub enum Action {
    /// Applies a value to the current namespace,
    ///
    With(String, Value),
    /// Extends the current namespace,
    ///
    Extend(Identifier),
    /// Configures a property,
    ///
    Config(Identifier, Property),
    /// Doc comment,
    ///
    Doc(String),
}

/// Returns an action that will apply a property,
///
pub fn with(name: impl Into<String>, value: impl Into<Value>) -> Action {
    Action::With(name.into(), value.into())
}

/// Returns an extend action,
///
pub fn extend(ident: &Identifier) -> Action {
    Action::Extend(ident.clone())
}

/// Returns a doc comment action,
///
pub fn doc(comment: impl Into<String>) -> Action {
    Action::Doc(comment.into().trim().to_string())
}

/// Returns a config action,
///
pub fn config(config_ident: &Identifier, config: &Property) -> Action {
    Action::Config(config_ident.clone(), config.clone())
}

/// Component for a list of actions to apply,
///
#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct ActionBuffer {
    /// Actions that are pending application,
    ///
    actions: Vec<Action>,
}

impl ActionBuffer {
    /// Returns a new empty action buffer,
    ///
    pub const fn new() -> Self {
        Self { actions: vec![] }
    }
    /// Pushes a new config to the buffer,
    ///
    pub fn push_config(&mut self, config_ident: &Identifier, _config: &Property) {
        self.actions.push(config(config_ident, _config));
    }

    pub fn config2(&self, entity: Entity, target: &mut impl Visitor) -> Result<(), Error> {
        for action in self.actions.iter() {
            match action {
                Action::Config(ident, prop) => {
                    let sigs = DispatchSignature::get_match(ident);
                    if sigs.len() > 1 {
                        warn!(
                            "Multiple signatures detected, {} {:#} -- {:?}",
                            ident, ident, sigs
                        );
                    }

                    match sigs.first() {
                        Some(sig) => {
                            match sig {
                                DispatchSignature::ExtendedProperty {
                                    config,
                                    name,
                                    extension,
                                    property: Some(property),
                                } => {
                                    trace!(
                                        config,
                                        name,
                                        extension,
                                        property,
                                        "Detected Extended Property Signature --"
                                    );
                                    if let Some(properties) = prop.as_properties() {
                                        for (name, prop) in properties
                                            .iter_properties()
                                            .filter(|(name, _)| *name != property)
                                        {
                                            let config_ext = format!("{config}.{extension}.{name}")
                                                .parse::<Identifier>()?;
                                            target.visit_property(name, prop);
                                            target.visit_extension(EntityVisitor::Owner(entity), &config_ext);
                                        }

                                        let config_prop = format!("{name}.{extension}.{property}")
                                            .parse::<Identifier>()?;
                                        let prop = properties.property(property).expect("should be a property since this is an extended property");
                                        target.visit_property(property, prop);
                                        target.visit_extension(EntityVisitor::Owner(entity), &config_prop);
                                    }
                                }
                                DispatchSignature::ExtendedProperty {
                                    config,
                                    name,
                                    extension,
                                    property: None,
                                } => {
                                    trace!(
                                        config,
                                        name,
                                        extension,
                                        "Detected Extended Property Signature --"
                                    );
                                    if let Some(properties) = prop.as_properties() {
                                        for (name, prop) in properties
                                            .iter_properties()
                                        {
                                            let config_ext = format!("{config}.{extension}.{name}")
                                                .parse::<Identifier>()?;
                                            target.visit_property(name, prop);
                                            target.visit_extension(EntityVisitor::Owner(entity), &config_ext);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        None => {
                            continue;
                        }
                    }
                }
                _ => {
                    continue;
                }
            }
        }

        Ok(())
    }

    /// Iterate over stored actions,
    ///
    pub fn iter_actions(&self) -> impl Iterator<Item = &Action> {
        self.actions.iter()
    }
}
