use reality::v2::command::export_toml;
use reality::v2::BuildLog;
use reality::v2::BuildRef;
use reality::v2::Compile;
use reality::v2::Compiler;
use reality::v2::Parser;
use reality::v2::Properties;
use reality::v2::Visitor;
use reality::v2::WorldWrapper;
use reality::Error;
use reality::Identifier;
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
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::ops::Deref;
use tracing::trace;

/// Example of a plugin framework compiler,
///
#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut compiler = Compiler::new().with_docs();
    let framework = compile_framework(&mut compiler)?;
    println!("Compiled framework: {:?}", framework);

    let mut framework = PluginFramework::new(framework);
    compiler.visit_last_build(&mut framework);
    println!("Configuring framework {:#?}", framework);
    export_toml(&mut compiler, ".test/plugin_framework.toml").await?;

    // Compile example usage of plugin framework
    let framework_usage = compile_example_usage(&mut compiler)?;
    println!("Compiled example usage: {:?}", framework_usage);

    // Add a plugin framework to the last build
    compiler.update_last_build(&mut framework.clone());

    // Run configure
    configure(compiler.as_mut());

    export_toml(&mut compiler, ".test/usage_example.toml").await?;
    Ok(())
}

/// Compiles the initial framework,
///
fn compile_framework(compiler: &mut Compiler) -> Result<Entity, Error> {
    let _ = Parser::new().parse(ROOT_RUNMD, compiler)?;

    compiler.compile()
}

/// Compiles the example usage of the framework,
///
fn compile_example_usage(compiler: &mut Compiler) -> Result<Entity, Error> {
    let _ = Parser::new().parse(EXAMPLE_USAGE, compiler)?;

    compiler.compile()
}

/// Runmd definition for a plugin framework,
///
const ROOT_RUNMD: &'static str = r##"
```runmd
+ .plugin                                   # A plugin root w/ common extensions for expressing a plugin
<> .path                                    # Indicates that a property will be a path
<> .map                                     # Indicates that a property will be a list of property names
<> .list                                    # Indicates that a property will be a list
<> .call                                    # Indicates that a property will be used as the input for a thunk_call

+ .plugin    Println                        # A plugin that prints text
<call>      .stdout                         # The plugin will print the value of the property to stdout
<call>      .stderr                         # The plugin will print the value of the property to stderr

+ .plugin    Process                        # A plugin that starts a program
: env       .symbol                         # Environment variables
<map>       .env                            # Property will be a list of environment variable names which are also property names
<call>      .process                        # The plugin will start a program where the name of the program is the property process
```
"##;

/// Runmd definition for usage of the plugin framework
///
const EXAMPLE_USAGE: &'static str = r##"
```runmd
+ .plugin                                               # Extending the framework by adding a new extension and plugin
<> .listen                                              # Indicates that a root will have a thunk_listen component

+ .plugin   Readln                                      # A plugin that reads text
<listen>    .stdin                                      # The plugin will use a thunk_listen to write to a property specified by the value of this property
```

```runmd app
+ .usage
<plugin> .println
: .stdout Hello World                                   # This message will be printed
: .stdout Hello World 2
: .stderr Hello Error World
: .stderr Hello Error World 2
<plugin.println>        .stdout     World Hello         # Can also be activated in one line
<plugin>                .process    cargo               # This process will be started
: RUST_LOG              .env        reality=trace
<plugin.readln>         .stdin      name                # This will read stdin and save the value to the property name
```
"##;

/// Struct for a plugin framework,
///
#[derive(Component, Debug, Clone)]
#[storage(HashMapStorage)]
struct PluginFramework {
    /// Build entity,
    ///
    _entity: Entity,
    /// Roots to filter by,
    ///
    roots: BTreeSet<String>,
    /// Registered identifier patterns to query for,
    ///
    patterns: Vec<String>,
    /// Config identifier patterns to query for,
    ///
    config_patterns: Vec<(String, String)>,
    /// Queue for configuring extensions,
    ///
    config_queue: VecDeque<Identifier>,
}

impl PluginFramework {
    const fn new(entity: Entity) -> Self {
        Self {
            _entity: entity,
            roots: BTreeSet::new(),
            patterns: vec![],
            config_patterns: vec![],
            config_queue: VecDeque::new(),
        }
    }

    fn patterns(&self) -> impl Iterator<Item = &String> {
        self.patterns.iter()
    }

    fn config_patterns(&self) -> impl Iterator<Item = &(String, String)> {
        self.config_patterns.iter()
    }

    fn check(&self, other: &Identifier) -> Option<(String, String)> {
        for (p, map) in self
            .patterns()
            .map(|p| (p, other.interpolate(p)))
            .filter(|(_, b)| b.is_some())
        {
            if let Some(map) = map {
                let name = &map["name"].to_lowercase();
                let property = &map["property"];
                // If the name is the same as the property, it's implied that its referencing itself
                if name == property {
                    return Some((p.clone(), format!(".{}.(input)", name)));
                } else {
                    // Otherwise, the pattern is {name}.{pattern}.(input)
                    return Some((
                        p.clone(),
                        format!(
                            ".{}.{}.(input)",
                            name,
                            property
                        ),
                    ));
                }
            }
        }

        None
    }
}

impl Visitor for PluginFramework {
    fn visit_object(&mut self, object: &reality::v2::states::Object) {
        object.as_root().map(|r| {
            self.visit_root(r);
        });
    }

    fn visit_extension(&mut self, identifier: &reality::Identifier) {
        // If this extension is part of a root we are configuring skip
        if self.roots.contains(&identifier.root()) {
            self.config_queue.push_back(identifier.clone());
            return;
        }

        println!("{:#} - {:?}", identifier, identifier);

        // This will update the identifier interpolation patterns to look for when applying extensions
        if let Some(root) = identifier
            .parent()
            // We just want the root
            .map(|p| p.deref().clone().flatten())
            // Since this is a framework, this root definition should be in the root block
            .filter(|p| p.parent().is_none())
        {
            if root.len() == 1 {
                let root = format!("{root}");
                let extension = format!("{identifier}");
                let pattern = format!("{root}.(name){extension}.(property)");
                self.patterns.push(pattern);
                self.roots.insert(root.trim_matches('.').to_string());
            } else if let Some(pattern) = self.check(identifier) {
                self.config_patterns.push(pattern);
            }
        }
    }
}

impl Compile for PluginFramework {
    fn compile<'a>(
        &self,
        build_ref: BuildRef<'a, Properties>,
    ) -> Result<BuildRef<'a, Properties>, Error> {
        build_ref
            .read(|r| {
                let owner = r.owner().clone();
                println!("Configuring -- {:#}", owner);

                // Check owner for a pattern match
                for (pattern, config_pattern) in self.config_patterns() {
                    if let Some(map) = owner.interpolate(config_pattern) {
                        println!("{pattern} --> {config_pattern} --> {:?}", map["input"]);
                        return Ok(());
                        // owner = owner.truncate(1)?;
                        // println!("Truncating {:#}", owner);
                    }
                }

                // Otherwise, check properties
                for (name, prop) in r.iter_properties() {
                    let ident = owner.branch(name)?;
                    for (pattern, config_pattern) in self.config_patterns() {
                        if let Some(messages) = prop.as_symbol_vec() {
                            for message in messages.iter() {
                                let ident = ident.branch(message)?;
                                if let Some(map) = ident.interpolate(config_pattern) {
                                    println!(
                                        "{pattern} --> {config_pattern} --> {:?}",
                                        map["input"]
                                    );
                                }    
                            }
                        }
                    }
                }

                Ok(())
            })
            .result()
    }
}

fn configure(world: &mut World) {
    world.exec(
        |(lazy_update, entities, logs, mut frameworks): (
            Read<LazyUpdate>,
            Entities,
            ReadStorage<BuildLog>,
            WriteStorage<PluginFramework>,
        )| {
            for (e, log, mut framework) in (&entities, &logs, frameworks.drain()).join() {
                println!("{:#?}", framework);
                while let Some(config) = framework.config_queue.pop_front() {
                    trace!("Searching build {:?} for {:#}", e, config);
                    let log = log.clone();
                    let config = config.clone();
                    let framework = framework.clone();
                    lazy_update.exec_mut(move |world| {
                        let mut wrapper = WorldWrapper::from(world);
                        log.find_ref(config, &mut wrapper).map(|r| {
                            framework.compile(r).ok().map(|_| {
                                trace!("Configured");
                            });
                        });
                    });
                }
            }
        },
    );

    world.maintain();
}
