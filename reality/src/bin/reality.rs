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
    );

    let _ = compiler
        .last_build_log()
        .expect("should have a build log");

    Ok(())
}

#[allow(unused_imports)]
mod framework {
    use reality::v2::{prelude::*, BuildLog, Visitor};
    use specs::VecStorage;

    pub static ROOT: &'static str = r##"
    ```runmd
    +  .cli         # Extensions for describing a cli command
    <> .command     # Extension to define a cli command
    <> .arg         # Extension to define a cli argument
    ```
    "##;

    pub static EXAMPLE: &'static str = r##"
    ```runmd examples
    + .cli export
    <command>   .export     # Creates a command `export`
    <arg>       .out        # Adds an argument called `out`
    ```
    "##;

    
    /// Struct 
    /// 
    pub struct Cli {
        command: clap::builder::Command
    }

    impl Visitor for Cli {
        fn visit_symbol(&mut self, name: &String, idx: Option<usize>, symbol: &String) {
            match name.as_str() {
                "about" => {
                    self.command = self.command.clone().about(symbol);
                }
                "version" => {
                    self.command = self.command.clone().version(symbol);
                }
                _ => {

                }
            }
        }
    }
}
