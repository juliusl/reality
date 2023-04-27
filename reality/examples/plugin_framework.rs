use std::collections::BTreeMap;

use reality::v2::{self, prelude::*, states::Object};
use tracing_subscriber::EnvFilter;

use crate::test_framework::Process;

// use test_framework::DispatchtestaExt;

/// Example of a plugin framework compiler,
///
/// A plugin adds functionality to a program.
///
/// This example will demonstrate runmd in this case.
///
/// Take for example we have 2 machines, machine A and machine B.
///
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .from_env()
                .expect("should be able to build from env variables")
                .add_directive(
                    "reality::v2::action=trace"
                        .parse()
                        .expect("should be able to parse tracing settings"),
                ),
        )
        .init();

    let mut args = std::env::args();
    let compiler = loop {
        if let Some(argument) = args.next() {
            match argument.as_str() {
                "toml" => {
                    break from_fs().await?;
                }
                _ => {
                    break from_runmd().await?;
                    //  break from_fs().await?;
                }
            }
        }
    };

    let log = compiler.last_build_log().unwrap();

    let mut config_map = BTreeMap::<String, String>::new();
    let mut config_actions = BTreeMap::<String, Property>::new();

    for (_, build) in compiler.compiled().state_vec::<v2::states::Build>() {
        let log = build.build_log;

        for (i, e) in log.index().iter() {
            println!("{:#} {:?}", i, e);
        }

        let matches = <test_framework::Process as Runmd>::Extensions::get_matches(&log);
        let mut process = test_framework::Process::new();
        let process = &mut process;

        for m in matches.iter() {
            println!("Process: {:#}--{:?}--{:?}", m.0, m.1, m.2);
            match &m.1 {
                test_framework::ProcessExtensions::PluginRoot {} => {
                    if let Some(o) = compiler.compiled().state::<Object>(m.2) {
                        process.visit_properties(o.properties());
                    }
                },
                test_framework::ProcessExtensions::PluginConfig { config, property } => {
                    let config_name = format!("Plugin.config_{}", config);
                    if let None = config_map.insert(property.clone(), config_name.to_string()) {
                        if let Some(o) = compiler.compiled().state::<Object>(m.2) {
                            let owner = o.properties().owner();
                            let key = format!("{}.{}", owner.root(), owner.subject());
                            println!("-- owner: {owner}");
                            for (n, prop) in o.properties().iter_properties() {
                                println!("-- {n} {:?}", prop);
                            }

                            config_actions.insert(config_name, Property::Properties(o.properties().clone().into()));
                        }
                    }
                },
                test_framework::ProcessExtensions::CliRoot {} => {
                    if let Some(o) = compiler.compiled().state::<Object>(m.2) {
                        process.visit_properties(o.properties());
                    }
                },
                test_framework::ProcessExtensions::CliConfig { config, property } => {
                    config_map.insert(property.clone(), config.clone());
                },
                // Check to see if a config exists for property
                test_framework::ProcessExtensions::Plugin { property: Some(property), .. } => {
                    *process = Process::new();
                    process.process = property.to_string();

                    if let Some(o) = compiler.compiled().state::<Object>(m.2) {
                        process.visit_properties(o.properties());

                        for (n, _) in o.properties().iter_properties() {
                            if let Some(config) = config_map.get(n).and_then(|c| config_actions.get(c)) {
                                println!("{n} config: {:?}", config);
                            }
                        }
                    }
                    println!("{:?}", process);
                },
                test_framework::ProcessExtensions::Cli { property: Some(..), .. } => {
                    if let Some(o) = compiler.compiled().state::<Object>(m.2) {
                        process.visit_properties(o.properties());

                        for (n, _) in o.properties().iter_properties() {
                            if let Some(config) = config_map.get(n).and_then(|c| config_actions.get(c)) {
                                println!("{n} config: {:?}", config);
                            }
                        }
                    }
                },
                _ => {

                }
            }
        }

        println!("{:#?}", config_actions);

        // for m in matches.iter() {
        //     match &m.0 {
        //         test_framework::ProcessExtensions::PluginRootConfig { .. } => {
        //             if let Some(o) = compiler.compiled().state::<Object>(m.1) {
        //                 let is_root = o.is_root();
        //                 if !is_root {
        //                     println!("Process: {:?}", m);
        //                 }
        //             }
        //         },
        //         test_framework::ProcessExtensions::CliRootConfig { .. } => {
        //             if let Some(o) = compiler.compiled().state::<Object>(m.1) {
        //                 let is_root = o.is_root();
        //                 if !is_root {
        //                     println!("Process: {:?}", m);
        //                 }
        //             }
        //         },
        //         _ => {
        //             println!("Process: {:?}", m);
        //         }
        //     }
        // }

        let matches = <test_framework::Println as Runmd>::Extensions::get_matches(&log);
        for m in matches.iter() {
            println!("Println: {:#}--{:?}--{:?}", m.0, m.1, m.2);
        }
    }

    let linker = Linker::new(test_framework::Process::new(), log.clone());

    for (idx, (id, e)) in log.index().iter().enumerate() {
        // println!("Build[{idx}]: {:#}", id);
        // println!("Build[{idx}]: {:?}", e);
        // compiler.as_ref().read_component::<Process>().get(*e).map(|e| {
        //     println!("{:?}", e);
        // });
        compiler.compiled().state::<Object>(*e).map(|o| {
            let map = o.ident().interpolate("#block#.(root).(?subject);");
            println!("block -- {:?}", map);

            let map = o
                .ident()
                .interpolate("#root#.plugin.(root).(?subject).(?property);");
            println!("process plugin extension -- {:?}", map);

            let is_block = o.is_block();
            let is_root = o.is_root();
            println!("is_block: {}, is_root: {}", is_block, is_root);
            let ident = o.ident();
            println!("{:#}", ident);
            for (name, prop) in o.properties().iter_properties() {
                let prop_ident = ident.branch(name).unwrap();
                println!("{name}->{:#}: {:?}", prop_ident, prop);
            }
        });
    }

    // let matches = DispatchSignature::get_matches(log.clone());
    // println!("{:#?}", matches);

    // log.find_ref::<ActionBuffer>("app.#block#.usage.#root#.plugin.println", &mut compiler)
    //     .unwrap()
    //     .transmute::<Properties>()
    //     .testa()?
    //     .enable_async()
    //     .call()
    //     .await?;

    Ok(())
}

async fn from_runmd() -> Result<Compiler> {
    let mut compiler = Compiler::new().with_docs();
    let framework = compile_example_framework(&mut compiler)?;
    println!("Compiled framework: {:?}", framework);

    // Configure framework from build
    let mut framework = Framework::new(framework);
    compiler.visit_last_build(&mut framework);
    println!("Configuring framework {:#?}", framework);
    export_toml(&mut compiler, ".test/plugin_framework.toml").await?;

    // Compile example usage runmd
    let framework_usage = compile_example_usage(&mut compiler)?;
    println!("Compiled example usage: {:?}", framework_usage);

    // Apply framework to last build
    compiler.update_last_build(&mut framework);
    println!("{:#?}", framework);

    // Configure to ingest and configure frameworks
    apply_framework!(compiler, test_framework::Process, test_framework::Println);
    compiler.as_mut().maintain();
    export_toml(&mut compiler, ".test/usage_example.toml").await?;

    Ok(compiler)
}

async fn from_fs() -> Result<Compiler> {
    let mut compiler = Compiler::new().with_docs();

    let framework = import_toml(&mut compiler, ".test/plugin_framework.toml").await?;
    let mut framework = Framework::new(framework);
    compiler.visit_last_build(&mut framework);

    let _ = import_toml(&mut compiler, ".test/usage_example.toml").await?;
    compiler.update_last_build(&mut framework);
    println!("{:#?}", framework);

    apply_framework!(compiler, test_framework::Process, test_framework::Println);
    Ok(compiler)
}

/// Compiles the initial framework,
///
fn compile_example_framework(compiler: &mut Compiler) -> Result<Entity> {
    let _ = Parser::new().parse(ROOT_RUNMD, compiler)?;

    // let _ = Parser::new()
    //     .parse_line("```runmd")?
    //     .parse_line("+  .plugin # A plugin root w/ common extensions for expressing a plugin")?
    //     .parse_line("<> .path   # Indicates that a property will be a path")?
    //     .parse_line("```")?
    //     .parse("", compiler);

    compiler.compile()
}

/// Compiles the example usage of the framework,
///
fn compile_example_usage(compiler: &mut Compiler) -> Result<Entity> {
    let _ = Parser::new().parse(EXAMPLE_USAGE, compiler)?;

    compiler.compile()
}

/// Runmd definition for a plugin framework,
///
const ROOT_RUNMD: &'static str = r##"
```runmd
+ .symbol   Plugin                          # Defining a symbol root called Plugin w/ common extensions
<> .path                                    # Indicates that a property will be a path
: canonical .false                          # Indicates whether the property should be a canonical path
<> .map                                     # Indicates that a property will be a list of property names
<> .list                                    # Indicates that a property will be a list
<> .call                                    # Indicates that a property will be used as the input for a thunk_call

+ .symbol Cli                               # Defining an extension called Cli
<> .command                                 # Adds a cli command

+ .plugin           Println                  # A plugin that prints text
<call>              .stdout                  # The plugin will print the value of the property to stdout
<call>              .stderr                  # The plugin will print the value of the property to stderr
<call>              .println                 # The plugin can be called on lists
<cli.command>       .test                    # The plugin will have a command call test        

+ .plugin    Process                        # A plugin that starts a program
: env       .symbol                         # Environment variables
: redirect  .symbol                         # Path to redirect stdout from the program to
<path>      .redirect                       # Should be a canonical path
: canonical .true
<map>       .env                            # Property will be a list of environment variable names which are also property names
<call>      .process                        # The plugin will start a program where the name of the program is the property process
```
"##;

/// Runmd definition for usage of the plugin framework
///
const EXAMPLE_USAGE: &'static str = r##"
```runmd
+                       .plugin                             # Extending the framework by adding a new extension and plugin
<>                      .listen                             # Indicates that a root will have a thunk_listen component

+                       .plugin   Readln                    # A plugin that reads text
<listen>                .stdin                              # The plugin will use a thunk_listen to write to a property specified by the value of this property
```

```runmd test app
+                       .symbol     Usage                   # Creating a simple root called Usage,
<plugin.println>        .stdout     World Hello             # Can also be activated in one line
<plugin.println>        .stderr     World Hello Error       # Can also be activated in one line
<cli.println>           .test       Test here               # Testing adding a command
<plugin>                .println
:                       .stdout     Hello World
:                       .stdout     Goodbye World
:                       .stderr     Hello World Error
:                       .stderr     Goodbye World Error
<plugin>                .process    cargo                   # This process will be started
: RUST_LOG              .env        reality=trace
:                       .redirect   .test/test.output
<plugin>                .process    python                  # This process will be started
:                       .redirect   .test2/test.output   
<plugin>                .println    pt2
:                       .stdout     Hello World 2
:                       .stdout     Goodbye World 2 
:                       .stderr     Hello World Error 2 
:                       .stderr     Goodbye World Error 2
<plugin.readln>         .stdin      name                    # This will read stdin and save the value to the property name
<> .start_usage                                             # Starts the usage example
```
"##;

#[allow(unused_imports)]
#[allow(dead_code)]
#[allow(unused_variables)]
pub mod test_framework {
    use reality::v2::prelude::*;
    use tracing_test::traced_test;

    #[derive(Clone, Debug, Default)]
    pub struct Plugin;

    impl Visitor for Plugin {
        fn visit_extension(&mut self, identifier: &Identifier) {
            println!("--- Plugin visited by: {:#}", identifier);
        }

        fn visit_property(&mut self, name: &String, property: &Property) {
            println!("--- Plugin visited by: {name}");
            println!("--- Plugin visited by: {:?}", property);
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Cli;

    impl Visitor for Cli {
        fn visit_property(&mut self, name: &String, property: &Property) {
            println!("--- Cli visited by: {name}");
            println!("--- Cli visited by: {:?}", property);
        }
    }

    #[thunk]
    pub trait TestA {
        fn testa(&self) -> reality::Result<()>;
    }

    impl TestA for Println {
        fn testa(&self) -> reality::Result<()> {
            println!("TestA impl for Println, {:?}", self);
            Ok(())
        }
    }

    #[derive(Runmd, Debug, Clone, Component)]
    #[storage(specs::VecStorage)]
    #[compile(ThunkCall, ThunkTestA)]
    pub struct Println {
        println: String,
        stderr: Vec<String>,
        stdout: Vec<String>,
        test: String,
        #[ext]
        plugin: Plugin,
        #[ext]
        cli: Cli,
    }

    #[async_trait]
    impl reality::v2::Call for Println {
        async fn call(&self) -> Result<Properties> {
            println!("entering");
            trace!("{:?}", self);
            for out in self.stdout.iter() {
                println!("{out}");
            }

            for err in self.stderr.iter() {
                eprintln!("{err}")
            }

            let mut props = Properties::new(Identifier::new());
            props["test"] = property_value("test written");
            Ok(props)
        }
    }

    impl Println {
        /// Should generate code like this,
        ///
        pub const fn new() -> Self {
            Self {
                println: String::new(),
                stderr: vec![],
                stdout: vec![],
                test: String::new(),
                plugin: Plugin {},
                cli: Cli {},
            }
        }
    }

    #[derive(Runmd, Debug, Clone, Component)]
    #[storage(specs::VecStorage)]
    #[compile(ThunkCall)]
    pub struct Process {
        pub process: String,
        pub redirect: String,
        pub rust_log: String,
        pub env: Vec<String>,
        #[ext]
        plugin: Plugin,
        #[ext]
        cli: Cli,
    }

    impl Process {
        pub const fn new() -> Self {
            Self {
                process: String::new(),
                redirect: String::new(),
                rust_log: String::new(),
                env: vec![],
                plugin: Plugin {},
                cli: Cli {},
            }
        }
    }

    #[async_trait]
    impl reality::v2::Call for Process {
        async fn call(&self) -> Result<Properties> {
            println!("Calling {}", self.process);
            println!("{:?}", self);

            Ok(Properties::default())
        }
    }
}
