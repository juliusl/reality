use reality::v2::{prelude::*, states::Object};
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
    for (idx, (id, e)) in log.index().iter().enumerate() {
        println!("Build[{idx}]: {:#}", id);
        println!("Build[{idx}]: {:?}", e);
        compiler.as_ref().read_component::<Process>().get(*e).map(|e| {
            println!("{:?}", e);
        });
        compiler.compiled().state::<Object>(*e).map(|o| {
            println!("Build[{idx}]: {}", o.properties());
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

+ .plugin    Println                        # A plugin that prints text
<call>      .stdout                         # The plugin will print the value of the property to stdout
<call>      .stderr                         # The plugin will print the value of the property to stderr
<call>      .println                        # The plugin can be called on lists

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
+ .plugin                                               # Extending the framework by adding a new extension and plugin
<> .listen                                              # Indicates that a root will have a thunk_listen component

+ .plugin   Readln                                      # A plugin that reads text
<listen>    .stdin                                      # The plugin will use a thunk_listen to write to a property specified by the value of this property
```

```runmd app
+                       .symbol     Usage               # Creating a simple root called Usage,
<plugin.println>        .stdout     World Hello         # Can also be activated in one line
<plugin.println>        .stderr     World Hello Error   # Can also be activated in one line
<plugin>    .println
: .stdout   Hello World
: .stdout   Goodbye World
: .stderr   Hello World Error
: .stderr   Goodbye World Error
<plugin>                .process    cargo               # This process will be started
: RUST_LOG              .env        reality=trace
:                       .redirect   .test/test.output
<plugin>                .process    python              # This process will be started
:                       .redirect   .test/test.output   
<plugin> .println pt2
: .stdout   Hello World 2
: .stdout   Goodbye World 2 
: .stderr   Hello World Error 2 
: .stderr   Goodbye World Error 2
<plugin.readln>         .stdin      name                # This will read stdin and save the value to the property name
<> .start_usage  # Starts the usage example

```
"##;

#[allow(dead_code)]
#[derive(Load)]
pub struct Test<'a> {
    identifier: &'a Identifier,
    properties: &'a Properties,
}

#[allow(unused_imports)]
#[allow(dead_code)]
#[allow(unused_variables)]
pub mod test_framework {
    use reality::v2::prelude::*;
    use reality::v2::property_value;
    use reality::v2::AsyncDispatch;
    use reality::v2::BuildLog;
    use reality::v2::DispatchRef;
    use reality::v2::Map;
    use reality::v2::MapWith;
    use reality::v2::Visitor;
    use reality::Identifier;
    use reality::Runmd;
    use specs::storage;
    use specs::DenseVecStorage;
    use specs::VecStorage;
    use std::collections::BTreeMap;
    use std::ops::Index;
    use std::ops::IndexMut;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tracing::trace;
    use tracing::Id;
    use tracing_test::traced_test;

    #[derive(Clone, Debug, Default)]
    pub struct Plugin;

    impl Visitor for Plugin {
        fn visit_extension(&mut self, entity: reality::v2::EntityVisitor, identifier: &Identifier) {
            println!("Plugin visited by: {:#}", identifier);
            println!("Plugin visited by: {:?}", entity);
        }

        fn visit_property(&mut self, name: &String, property: &Property) {
            println!("Plugin visited by: {name}");
            println!("Plugin visited by: {:?}", property);
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

    /*
    struct Println {
       test_config --> test.config.#root#


       ```
       + .println test_config
       + .println
       ```
    }
     */

    impl Println {
        /// Should generate code like this,
        ///
        /// ```
        /// fn dispatch(&self, r...) -> ... {
        ///     let s = Self::new();
        ///     r.store(s)?;
        /// }
        /// ```
        ///
        pub const fn new() -> Self {
            Self {
                println: String::new(),
                stderr: vec![],
                stdout: vec![],
                test: String::new(),
                plugin: Plugin{},
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
    }

    impl Process {
        pub const fn new() -> Self {
            Self {
                process: String::new(),
                redirect: String::new(),
                rust_log: String::new(),
                env: vec![],
                plugin: Plugin{},
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

    const DISPATCH_ROOT: &'static str = "#block#.#root#.(root).(ext).(name).(prop)";

    const DISPATCH_ROOT_CONFIG: &'static str =
        "#block#.#root#.(root).(config).(ext).(name).(?prop)";

    const DISPATCH_ROOT_EXT: &'static str = "#block#.#root#.(root).(ext);";

    #[derive(Component)]
    #[storage(VecStorage)]
    struct PluginFramework(DispatchSignature);

    impl<'b> Dispatch for PluginFramework {
        fn dispatch<'a>(&self, dispatch_ref: DispatchRef<'a, Properties>) -> DispatchResult<'a> {
            dispatch_ref
                .transmute::<BuildLog>()
                .read(|p| {
                    let matches = DispatchSignature::get_matches(p.clone());

                    Ok(())
                })
                .transmute()
                .result()
        }
    }

    impl Test {}

    struct Test;

    impl Dispatch for Test {
        fn dispatch<'a>(&self, dispatch_ref: DispatchRef<'a, Properties>) -> DispatchResult<'a> {
            if let Ok(accepted) = dispatch_ref
                .transmute::<Identifier>()
                .read(|id| {
                    if let Some(map) = id.interpolate("#block#.#root#.(root).Test.(ext).(?prop)") {
                        Ok(())
                    } else {
                        Err(Error::skip())
                    }
                })
                .result()
            {
                accepted.read(|_| Ok(()));
                Err(Error::skip())
            } else {
                Err(Error::not_implemented())
            }
        }
    }

    #[async_trait]
    impl AsyncDispatch for Test {
        async fn async_dispatch<'a, 'b>(
            &'a self,
            dispatch_ref: DispatchRef<'b, Properties>,
        ) -> DispatchResult<'b> {
            Ok(dispatch_ref)
        }
    }

    #[derive(Component)]
    #[storage(VecStorage)]
    struct Usage {}

    impl Usage {
        /// Dispatches on #block#.#root#.usage.start_usage,
        ///
        fn start_usage<'a>(&mut self) -> Result<()> {
            Ok(())
        }

        /// Dispatches on #block#.#root#.usage.read_usage,
        ///
        fn read_usage<'a>(&self) -> Result<()> {
            Ok(())
        }

        fn _dispatch_start_usage<'a>(
            dispatch_ref: DispatchRef<'a, Properties>,
        ) -> DispatchRef<'a, Properties> {
            dispatch_ref
                .transmute::<Self>()
                .write(|s| s.start_usage())
                .transmute()
        }
    }
}
