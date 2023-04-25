use reality::v2::{prelude::*, Documentation};
use tracing_subscriber::EnvFilter;

/// Commands,
///
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .from_env()
                .expect("should be able to build from env variables")
                .add_directive(
                    "reality::v2=info"
                        .parse()
                        .expect("should be able to parse tracing settings"),
                ),
        )
        .init();

    let mut compiler = Compiler::new().with_handler(|packet: Packet| {
        let Packet { block_identifier, identifier, keyword, actions } = packet;

        println!("-------------------------");
        println!("packet.block   : {:#}", block_identifier);
        println!("packet.ident   : {:#}", identifier);
        println!("packet.keyword : {:?}", keyword);
        println!("packet.actions : {:#?}", actions);
        println!("-------------------------");

        Ok(())
    });
    // let framework = import_toml(&mut compiler, ".test/cli_framework.toml").await?;

    let parser = Parser::new();
    let parser = parser.parse(framework::ROOT, &mut compiler)?;
    let framework = compiler.compile()?;
    let mut framework = Framework::new(framework);
    compiler.visit_last_build(&mut framework);
    export_toml(&mut compiler, ".test/cli_framework.toml").await?;

    parser.parse(framework::EXAMPLE, &mut compiler)?;
    compiler.compile()?;
    compiler.update_last_build(&mut framework);
    println!("{:#?}", framework);

    apply_framework!(compiler, framework::Test);
    compiler.as_mut().maintain();
    export_toml(&mut compiler, ".test/reality-examples.toml").await?;

    let log = compiler.last_build_log().unwrap();
    for (idx, (id, e)) in log.index().iter().enumerate() {
        println!("BuildLog[{idx}]: {:#}", id);
        println!("BuildLog[{idx}]: {:?}", e);
        compiler.compiled().state::<Object>(*e).map(|o| {
            println!("BuildLog[{idx}]: {}", o.properties());
        });

        println!("----------------------------------------------");
        compiler
            .as_ref()
            .read_component::<framework::Test>()
            .get(*e)
            .map(|e| {
                let mut command = e.cli.command.clone();
                command.build();
                command.print_help().ok();
            });
    }
    Ok(())
}

#[allow(unused_imports)]
mod framework {
    use reality::v2::{prelude::*, BuildLog, Visitor};
    use specs::VecStorage;

    pub static ROOT: &'static str = r##"
    ```runmd
    +  .cli                 # Extensions for describing a cli 
    <> .command             # Command extension configures a `clap::Command`
    : about     .symbol     # A short description of the command
    : version   .symbol     # A version string for this command
    : author    .symbol     # The author of this command

    + .cli Test
    <command>   .test 
    : .about    This is a test command
    : .version  v1.0.0
    : .author   Test Author
    ```
    "##;

    pub static EXAMPLE: &'static str = r##"
    ```runmd examples
    + .root
    <cli> .test 
    ```
    "##;

    #[derive(Runmd, Clone, Component, Debug)]
    #[storage(HashMapStorage)]
    pub struct Test {
        test: String,
        about: String,
        #[ext]
        pub cli: Cli,
    }

    impl Test {
        pub fn new() -> Test {
            Test {
                test: String::new(),
                about: String::new(),
                cli: Cli::new(),
            }
        }
    }

    /// Struct
    ///
    #[derive(Clone, Debug)]
    pub struct Cli {
        pub command: clap::builder::Command,
    }

    impl Cli {
        pub fn new() -> Self {
            Cli {
                command: clap::builder::Command::new(""),
            }
        }
    }

    #[dispatch_signature]
    enum CliSignatures {
        /// Signature of a command from `test` cli,
        ///
        #[interpolate("test.command.(name)")]
        TestCLICommand,
    }

    impl Visitor for Cli {
        fn visit_extension(&mut self, entity: reality::v2::EntityVisitor, identifier: &Identifier) {
            println!("Cli visited by ext: {:#}", identifier);
            println!("Cli visited by ext: {:?}", entity);

            let matches = CliSignatures::get_match(identifier);

            for m in matches.iter() {
                match m {
                    CliSignatures::TestCLICommand { name } => {
                        self.command = self.command.clone().name(name);
                    }
                }
            }
        }

        fn visit_symbol(&mut self, name: &String, _: Option<usize>, symbol: &String) {
            println!("Cli visited by prop: {name}");
            println!("Cli visited by prop: {:?}", symbol);
            match name.as_str() {
                "about" => {
                    self.command = self.command.clone().about(symbol);
                }
                "version" => {
                    self.command = self.command.clone().version(symbol);
                }
                "author" => {
                    self.command = self.command.clone().author(symbol);
                }
                _ => {}
            }
        }
    }
}
