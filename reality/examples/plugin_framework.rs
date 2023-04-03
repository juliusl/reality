use reality::state::Load;
use reality::state::Provider;
use reality::v2::command::export_toml;
use reality::v2::framework::configure;
use reality::v2::Compiler;
use reality::v2::Framework;
use reality::v2::Parser;
use reality::v2::Properties;
use reality::Error;
use reality::Identifier;
use reality::Load;
use specs::prelude::*;
use specs::Entity;
use specs::ReadStorage;
use specs::SystemData;
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
        .compact()
        .init();

    // Compile the example framework runmd
    let mut compiler = Compiler::new().with_docs();
    let testent = compiler
        .as_mut()
        .create_entity()
        .with(Identifier::new())
        .with(Properties::default())
        .build();
    let state = compiler
        .as_mut()
        .system_data::<TestSystemData>()
        .state::<Test>(testent)
        .expect("should exist");

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
    compiler.update_last_build(&mut framework.clone());

    // Configure to digest frameworks
    configure(compiler.as_mut());

    export_toml(&mut compiler, ".test/usage_example.toml").await?;
    Ok(())
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
<plugin> .println
: .stdout Hello World                                   # This message will be printed
: .stdout Hello World 2
: .stderr Hello Error World
: .stderr Hello Error World 2
<plugin.println>        .stdout     World Hello         # Can also be activated in one line
<plugin>                .process    cargo               # This process will be started
: RUST_LOG              .env        reality=trace
:                       .redirect   .test/test.output
<plugin.readln>         .stdin      name                # This will read stdin and save the value to the property name
```
"##;

#[derive(SystemData)]
pub struct TestSystemData<'a> {
    entities: Entities<'a>,
    identifiers: ReadStorage<'a, Identifier>,
    properties: ReadStorage<'a, Properties>,
}

#[derive(Load)]
pub struct Test<'a> {
    identifier: &'a Identifier,
    properties: &'a Properties,
}

impl<'a> AsRef<Entities<'a>> for TestSystemData<'a> {
    fn as_ref(&self) -> &Entities<'a> {
        &self.entities
    }
}

impl<'a> Provider<'a, TestFormat<'a>> for TestSystemData<'a> {
    fn provide(&'a self) -> TestFormat<'a> {
        (&self.identifiers, &self.properties)
    }
}
