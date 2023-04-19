use reality::v2::prelude::*;

/// Commands,
///
#[tokio::main]
async fn main() -> Result<()> {
    let mut compiler = Compiler::new().with_docs();
    let parser = Parser::new();
    let parser = parser.parse(framework::ROOT, &mut compiler)?;
    let framework = compiler.compile()?;
    let mut framework = Framework::new(framework);
    compiler.visit_last_build(&mut framework);

    parser.parse(framework::EXAMPLE, &mut compiler)?;
    compiler.compile()?;
    compiler.update_last_build(&mut framework);

    apply_framework!(
        compiler, 
        framework::Cli, 
        framework::Shell
    );

    let log = compiler.last_build_log().expect("should have a build log");
    println!("{}", log);
    println!("{:#?}", framework);

    log.find_ref::<framework::Shell>(
        "#block#.#root#.main",
        &mut compiler,
    )
    .expect("Should have a main root to start")
    .read(|b| {
        println!("{:#?}", b);
        Ok(())
    });

    Ok(())
}

#[allow(unused_imports)]
mod framework {
    use reality::v2::{prelude::*, BuildLog, Visitor};
    use specs::VecStorage;

    pub static ROOT: &'static str = r##"
    ```runmd
    +  .cli                 # Root for designing cli extensions
    <> .command             # Command extension
    :   alias   .symbol     # Aliases to use for this command
    <>  .arg                # Argument extension
    :   flag    .false      # True if this argument is a flag
    :   pos     .int        # Argument position 

    +  .cli         shell           # CLI interface for a shell
    : shell         .symbol         # Input to pass to the shell
    : arg           .symbol         # Map of name/argument values 
    : <command>     .shell          : .alias sh
    : <arg>         .arg

    ```
    "##;

    pub static EXAMPLE: &'static str = r##"
    ```runmd examples
    +         .example        # Example Usage 
    <cli>     .shell  help    # Prints help information, ex. shell help --name example 
    : name    .arg            # Name to find help for
    : title   .arg            # Title to find help for
    <cli>     .shell  view    # Views a the properties of entities, 
    : pattern .arg            # Pattern to query for entities,
    ```
    "##;

    #[derive(Runmd, Component, Clone, Default)]
    #[storage(VecStorage)]
    #[compile]
    pub struct Example {
        /// Help shell command for example
        /// 
        help: Option<Shell>,
    }

    #[derive(Runmd, Component, Clone, Debug, Default)]
    #[storage(VecStorage)]
    pub struct Cli {
        command: Command,
        arg: (),
    }

    impl Cli {
        pub const fn new() -> Self {
            Self {
                command: Command::new(),
                arg: (),
            }
        }
    }

    /// Command extension settings,
    /// 
    #[derive(Runmd, Component, Clone, Debug, Default)]
    #[storage(VecStorage)]
    pub struct Command {
        /// Command name,
        /// 
        command: String,
        /// Aliases for this command,
        /// 
        alias: Vec<String>,
    }

    impl Command {
        /// Creates a new empty command,
        /// 
        pub const fn new() -> Self {
            Self { command: String::new(), alias: vec![] }
        }
    }

    /// 
    #[derive(Runmd, Clone, Default, Debug, Component)]
    #[storage(VecStorage)]
    #[compile]
    pub struct Shell {
        /// Command to execute against Shell,
        /// 
        shell: String,
        /// Argument to indicate debug mode,
        /// 
        debug: bool,
        /// CLI Settings for shell root,
        /// 
        #[root]
        cli: Cli,
    }

    #[async_trait]
    impl Call for Shell {
        async fn call(&self) -> Result<Properties> {
            todo!()
        }
    }

    impl Shell {
        pub const fn new() -> Self {
            Self {
                shell: String::new(),
                debug: false,
                cli: Cli::new(),
            }
        }
    }
}
