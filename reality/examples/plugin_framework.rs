use reality::state::Provider;
use reality::v2::command::export_toml;
use reality::v2::framework::configure;
use reality::v2::property_value;
use reality::v2::Compiler;
use reality::v2::Framework;
use reality::v2::Parser;
use reality::v2::Properties;
use reality::Error;
use reality::Identifier;
use reality_derive::Load;
use specs::Builder;
use specs::Entity;
use specs::WorldExt;
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

    // Compile the example framework runmd
    let mut compiler = Compiler::new().with_docs();
    let mut properties = Properties::default();
    properties["test"] = property_value(true);
    let testent = compiler
        .as_mut()
        .create_entity()
        .with(Identifier::new())
        .with(properties)
        .build();

    let state = compiler
        .as_mut()
        .system_data::<TestSystemData>()
        .state::<Test>(testent)
        .expect("should exist")
        .properties["test"]
        .is_enabled();
    assert!(state);

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

#[allow(dead_code)]
#[derive(Load)]
pub struct Test<'a> {
    identifier: &'a Identifier,
    properties: &'a Properties,
}

#[allow(unused_imports)]
#[allow(dead_code)]
mod tests {
    use reality::v2::{Apply, Config, property_list};
    use reality::Error;
    use reality_derive::Apply;
    use reality_derive::Config;
    use std::path::PathBuf;

    use reality::{
        v2::{property_value, Property},
        Identifier,
    };

    #[derive(Config)]
    struct Test {
        #[config(config_name)]
        name: String,
        is_test: bool,
        n: usize,
    }

    impl Test {
        const fn new() -> Self {
            Self {
                name: String::new(),
                is_test: false,
                n: 0,
            }
        }
    }

    #[derive(Config, Apply)]
    pub struct Plugin {
        #[apply]
        pub path: PathConfig,
        pub map: (),
        pub list: (),
        #[apply]
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

    #[derive(Config)]
    pub struct PathConfig {
        canonical: bool,
    }

    #[derive(Config)]
    pub struct CallConfig {
        test: bool,
    }

    impl Apply for CallConfig {
        fn apply(&self, _: impl AsRef<str>, property: &Property) -> Result<Property, Error> {
            println!("Applying call config");
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

    #[derive(Config)]
    pub struct Println {
        stderr: Vec<String>,
        stdout: Vec<String>,
        #[root]
        plugin: Plugin,
    }

    #[derive(Config)]
    pub struct Process {
        redirect: String,
        #[root]
        plugin: Plugin,
    }

    #[test]
    fn test_config() {
        let mut test = Test::new();

        let ident = "test.a.b.name".parse::<Identifier>().unwrap();
        let property = property_value("test_name");
        test.config(&ident, &property).unwrap();

        let ident = "test.a.b.is_test".parse::<Identifier>().unwrap();
        let property = property_value(true);
        test.config(&ident, &property).unwrap();

        let ident = "test.a.b.n".parse::<Identifier>().unwrap();
        let property = property_value(100);
        test.config(&ident, &property).unwrap();

        assert_eq!("Config: test_name", test.name.as_str());
        assert_eq!(true, test.is_test);
        assert_eq!(100, test.n);

        let mut plugin = Plugin::new();
        plugin.path.canonical = true;
        let _ = plugin
            .apply("path", &Property::Empty)
            .expect_err("should return an error");

        let stderr_ident = ".plugin.Println.call.stderr".parse::<Identifier>().unwrap();

        let mut println = Println {
            stderr: vec![],
            stdout: vec![],
            plugin: Plugin::new()
        };

        let list = property_list(vec!["Hello world", "Hello world 2"]);
        let _ = println.config(&stderr_ident, &list).unwrap();

        println!("{:?}", println.stderr);

        let redirect_ident = ".plugin.Process.path.redirect".parse::<Identifier>().unwrap();

        let mut plugin = Plugin::new();
        plugin.path.canonical = true;

        let mut process = Process {
            redirect: String::new(),
            plugin
        };

        let path = property_value(".random/");
        let _ = process.config(&redirect_ident, &path).expect_err("should return an error");
        
        let path = property_value(".test/");
        let _ = process.config(&redirect_ident, &path).unwrap();
    }

    fn config_name(_: &Test, _: &Identifier, property: &Property) -> Result<String, Error> {
        Ok(format!(
            "Config: {}",
            property.as_symbol().unwrap_or(&String::default())
        ))
    }

}
