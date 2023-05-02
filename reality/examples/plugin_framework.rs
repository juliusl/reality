use reality::v2::prelude::*;
use tracing_subscriber::EnvFilter;

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
                    "reality::v2=trace"
                        .parse()
                        .expect("should be able to parse tracing settings"),
                ),
        )
        .init();

    let mut args = std::env::args();
    let mut compiler = loop {
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

    compiler.link(test_framework::Process::new()).unwrap();
    compiler.link(test_framework::Println::new()).unwrap();

    let log = compiler.last_build_log().unwrap();

    for (_, m, e) in test_framework::ProcessExtensions::get_matches(&log) {
        match m {
            test_framework::ProcessExtensions::Plugin { .. } => {
                compiler.as_ref().read_component::<test_framework::Process>().get(e).map(|p| {
                    println!("post-link -- {:#?}", p);
                });
            },
            _ => {}
        }
    }

    for (_, m, e) in test_framework::PrintlnExtensions::get_matches(&log) {
        match m {
            test_framework::PrintlnExtensions::Plugin { .. } => {
                compiler.as_ref().read_component::<test_framework::Println>().get(e).map(|p| {
                    println!("post-link -- {:#?}", p);
                });
            },
            _ => {}
        }
    }

    Ok(())
}

async fn from_runmd() -> Result<Compiler> {
    let mut compiler = Compiler::new().with_docs();
    let framework = compile_example_framework(&mut compiler)?;
    println!("Compiled framework: {:?}", framework);
    let framework_usage = compile_example_usage(&mut compiler)?;
    println!("Compiled example usage: {:?}", framework_usage);
    compiler.as_mut().maintain();
    export_toml(&mut compiler, ".test/usage_example.toml").await?;

    Ok(compiler)
}

async fn from_fs() -> Result<Compiler> {
    let mut compiler = Compiler::new().with_docs();

    let _ = import_toml(&mut compiler, ".test/plugin_framework.toml").await?;
    let _ = import_toml(&mut compiler, ".test/usage_example.toml").await?;

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
: HOME_DIR              .env        /etc/acr
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
    use std::collections::BTreeMap;

    use reality::{v2::prelude::*, Value};
    use tracing_test::traced_test;

    #[derive(Clone, Debug, Default)]
    pub struct Plugin {
        list: (),
        properties: Properties,
    }

    impl Plugin {
        /// Returns read-only properties of values found in properties,
        /// 
        fn map(&self, properties: &Vec<String>) -> Property {
            let mut output = Properties::empty();
            for name in properties {
                if let Some(prop) = self.properties.property(name) {
                    output.set(name, prop.clone());
                }
            }

            Property::Properties(output.into())
        }
    }

    impl Visitor for Plugin {
        fn visit_extension(&mut self, identifier: &Identifier) {
            println!("--- Plugin visited by: {:#}", identifier);
        }

        fn visit_property(&mut self, name: &str, property: &Property) {
            println!("--- Plugin visited by: {name}");
            println!("--- Plugin visited by: {:?}", property);

            self.properties.visit_property(name, property);
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Cli;

    impl Visitor for Cli {
        fn visit_property(&mut self, name: &str, property: &Property) {
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

    impl Visit<&Plugin> for Println {
        fn visit(&self, context: &Plugin, visitor: &mut impl Visitor) -> Result<()> {
            println!("visiting -- {}", self.println);
            for (name, prop) in context.properties.iter_properties() {
                println!("visiting -- {name} -- {:#?}", prop);
                visitor.visit_property(name, prop);
            }

            Ok(())
        }
    }

    impl Visit<&Cli> for Println {
        fn visit(&self, context: &Cli, visitor: &mut impl Visitor) -> Result<()> {
            Ok(())
        }
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
                plugin: Plugin { list: (), properties: Properties::empty() },
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
        #[config(rename="RUST_LOG")]
        pub rust_log: String,
        #[config(rename="env", ext=plugin.map)]
        pub env: Vec<String>,
        #[ext]
        plugin: Plugin,
        #[ext]
        cli: Cli,
    }

    impl Visit<&Cli> for Process {
        fn visit(&self, context: &Cli, visitor: &mut impl Visitor) -> Result<()> {
            Ok(())
        }
    }

    impl Visit<&Plugin> for Process {
        fn visit(&self, context: &Plugin, visitor: &mut impl Visitor) -> Result<()> {
            println!("visiting -- {}", self.process);
            for (name, prop) in context.properties.iter_properties() {
                println!("visiting -- {name} -- {:#?}", prop);
                visitor.visit_property(name, prop);
            }
            
            Ok(())
        }
    }

    impl Process {
        pub const fn new() -> Self {
            Self {
                process: String::new(),
                redirect: String::new(),
                rust_log: String::new(),
                env: vec![],
                plugin: Plugin { list: (), properties: Properties::empty() },
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

    #[test]
    fn test_visit_process() {
        let process = Process {
            process: String::from("test"),
            redirect: String::from(".test/test.log"),
            rust_log: String::from("trace"),
            env: vec![String::from("a"), String::from("b")],
            plugin: Plugin,
            cli: Cli,
        };

        let mut props = Properties::empty();
        process.visit((), &mut props).unwrap();

        assert_eq!("test", props["process"].as_symbol().unwrap().as_str());
        assert_eq!(".test/test.log", props["redirect"].as_symbol().unwrap().as_str());
        assert_eq!("trace", props["RUST_LOG"].as_symbol().unwrap().as_str());
        println!("{:#}", props);

        let m = ProcessExtensions::PluginConfig { config: (), property: () };
    }
}
