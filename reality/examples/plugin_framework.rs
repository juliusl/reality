use reality::apply_framework;
use reality::v2::command::export_toml;
use reality::v2::command::import_toml;
use reality::v2::BuildRef;
use reality::v2::Call;
use reality::v2::Compiler;
use reality::v2::Framework;
use reality::v2::Parser;
use reality::v2::Properties;
use reality::v2::Runmd;
use reality::v2::ThunkCall;
use reality::Error;
use reality::Identifier;
use reality_derive::Load;
use specs::Entity;
use tracing_subscriber::EnvFilter;

/// Example of a plugin framework compiler,
///
#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .from_env()
                .expect("should be able to build from env variables")
                .add_directive(
                    "reality::v2::framework=trace"
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
                    // break from_fs().await?;
                }
            }
        }
    };

    let log = compiler.last_build_log().unwrap();

    for (_, _, e) in log.search_index("plugin.process") {
        let build_ref = BuildRef::<ThunkCall>::new(*e, &mut compiler);
        build_ref
            .enable_async()
            .read(|tc| {
                let tc = tc.clone();
                async move {
                    tc.call().await?;
                    Ok(())
                }
            })
            .await;
    }

    for (_, _, e) in log.search_index("plugin.println") {
        let build_ref = BuildRef::<ThunkCall>::new(*e, &mut compiler);
        build_ref
            .enable_async()
            .map_with::<test_framework::Println, _>(|call, println| {
                reality::v2::call_config_into(call.clone(), println.clone())
            })
            .await
            .disable_async()
            .transmute::<test_framework::Println>()
            .read(|p| {
                println!("{:?}", p);
                Ok(())
            });
    }

    Ok(())
}

async fn from_runmd() -> Result<Compiler, Error> {
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

async fn from_fs() -> Result<Compiler, Error> {
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
fn compile_example_framework(compiler: &mut Compiler) -> Result<Entity, Error> {
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
+ .usage
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
```
"##;

#[allow(dead_code)]
#[derive(Load)]
pub struct Test<'a> {
    identifier: &'a Identifier,
    properties: &'a Properties,
}

pub mod test_framework {
    use reality::{
        v2::{prelude::*, property_value, BuildLog, BuildRef, ThunkCall, WorldWrapper},
        Identifier, Runmd,
    };
    use specs::{Entities, Join, LazyUpdate, Read, ReadStorage, WorldExt, WriteStorage};
    use std::path::PathBuf;
    use tokio::task::JoinHandle;

    #[derive(Config, Clone, Apply, Debug, Default)]
    pub struct Plugin {
        pub path: PathConfig,
        pub map: (),
        pub list: (),
        pub call: CallConfig,
    }

    impl Plugin {
        const fn new() -> Self {
            Self {
                path: PathConfig { canonical: false },
                map: (),
                list: (),
                call: CallConfig { test: false },
            }
        }
    }

    #[derive(Config, Clone, Debug, Default)]
    pub struct PathConfig {
        canonical: bool,
    }

    #[derive(Config, Clone, Debug, Default)]
    pub struct CallConfig {
        test: bool,
    }

    impl Apply for CallConfig {
        fn apply(&self, name: impl AsRef<str>, property: &Property) -> Result<Property, Error> {
            println!("Applying call config: {} -- {:?}", name.as_ref(), property);
            Ok(property.clone())
        }
    }

    impl Apply for PathConfig {
        fn apply(&self, _: impl AsRef<str>, property: &Property) -> Result<Property, Error> {
            if self.canonical {
                if let Some(path) = property.as_symbol().map(|s| PathBuf::from(s)) {
                    path.canonicalize()?;
                } else {
                    return Err("Could not canonicalize property".into());
                }
            }

            Ok(property.clone())
        }
    }

    #[derive(Runmd, Config, Debug, Clone, Component)]
    #[storage(specs::VecStorage)]
    #[compile(Call)]
    pub struct Println {
        println: String,
        stderr: Vec<String>,
        stdout: Vec<String>,
        test: String,
        #[root]
        plugin: Plugin,
    }

    #[async_trait]
    impl Call for Println {
        async fn call(&self) -> Result<Properties, Error> {
            println!("{:?}", self);
            for out in self.stdout.iter() {
                println!("{out}");
            }

            for err in self.stderr.iter() {
                println!("{err}")
            }

            let mut props = Properties::new(Identifier::new());
            props["test"] = property_value("test written");
            Ok(props)
        }
    }

    impl Println {
        pub const fn new() -> Self {
            Self {
                println: String::new(),
                stderr: vec![],
                stdout: vec![],
                test: String::new(),
                plugin: Plugin::new(),
            }
        }
    }

    #[derive(Runmd, Config, Debug, Clone, Component)]
    #[storage(specs::VecStorage)]
    #[compile(ThunkCall)]
    pub struct Process {
        process: String,
        redirect: String,
        #[root]
        plugin: Plugin,
    }

    impl Process {
        pub const fn new() -> Self {
            Self {
                process: String::new(),
                redirect: String::new(),
                plugin: Plugin::new(),
            }
        }
    }

    #[async_trait]
    impl Call for Process {
        async fn call(&self) -> Result<Properties, Error> {
            println!("Calling {}", self.process);
            println!("{:?}", self);
            Ok(Properties::default())
        }
    }

    struct CompileSystem<T: Compile + Clone>(T);
    impl<'a> specs::System<'a> for CompileSystem<Process> {
        type SystemData = (
            Entities<'a>,
            Read<'a, LazyUpdate>,
            ReadStorage<'a, BuildLog>,
        );

        fn run(&mut self, (entities, lazy_update, logs): Self::SystemData) {
            for (_, log) in (&entities, &logs).join() {
                let log = log.clone();
                let clone = self.0.clone();
                lazy_update.exec_mut(move |world| {
                    let mut wrapper = WorldWrapper::from(world);

                    for (id, _, entity) in log.search_index("plugin.process.(input)") {
                        let build_ref = BuildRef::<Properties>::new(*entity, &mut wrapper);

                        if let Ok(_) = clone.compile(build_ref) {
                            println!("plugin.process, {:#}, {:?}", id, entity);
                        }
                    }
                });
            }
        }
    }

    struct ProcessCall;

    #[derive(Component)]
    #[storage(specs::VecStorage)]
    struct ProcessCallTask(JoinHandle<Result<Properties, Error>>);

    #[derive(Component)]
    #[storage(specs::VecStorage)]
    struct ProcessCallResult(Result<Properties, Error>);

    impl<'a> specs::System<'a> for ProcessCall {
        type SystemData = (
            Entities<'a>,
            Read<'a, LazyUpdate>,
            Read<'a, Option<tokio::runtime::Handle>>,
            ReadStorage<'a, Process>,
            ReadStorage<'a, ThunkCall>,
            WriteStorage<'a, ProcessCallTask>,
            WriteStorage<'a, ProcessCallResult>,
        );

        fn run(
            &mut self,
            (entities, lazy_update, handle, processes, calls, mut tasks, mut results): Self::SystemData,
        ) {
            if let Some(handle) = handle.as_ref() {
                for (e, _, call) in (&entities, &processes, &calls).join() {
                    if results.contains(e) {
                        continue;
                    }

                    if let Some(task) = tasks.remove(e) {
                        if task.0.is_finished() {
                            let handle = handle.clone();
                            results
                                .insert(
                                    e,
                                    ProcessCallResult(handle.block_on(async { task.0.await? })),
                                )
                                .ok();
                        } else {
                            tasks.insert(e, task).ok();
                        }
                    } else {
                        let handle = handle.clone();
                        let call = call.clone();
                        lazy_update.exec_mut(move |world| {
                            let task = handle.spawn(async move { call.call().await });
                            let task = ProcessCallTask(task);
                            world.write_component().insert(e, task).ok();
                        });
                    }
                }
            }
        }
    }
}
