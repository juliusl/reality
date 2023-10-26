use univerux::terminal::TerminalApp;
use univerux::terminal::Terminal;
use univerux::ProjectLoop;
use univerux::AppType;
use reality::Shared;
use reality::Project;

/// Minimal example for starting a new terminal repl interaction,
/// 
fn main() {
    BlankRepl::start_interaction(
        ProjectLoop::new(Project::new(Shared::default())), 
        Terminal
    );
}

struct BlankRepl;

impl AppType<Shared> for BlankRepl {
    fn create(
        _: intrglctive::ProjectLoop<Shared>,
    ) -> Self {
        BlankRepl
    }

    fn initialize_storage() -> Shared {
        Shared::default()
    }
}

impl TerminalApp<Shared> for BlankRepl {
    fn parse_command(&mut self) -> clap::Command {
        // If using derive -- 
        // clap::CommandFactory::command();
        // clap::CommandFactory::command_for_update()

        clap::builder::Command::new("test")
            .multicall(true)
            .subcommand(clap::builder::Command::new("ping"))
            .subcommand(clap::builder::Command::new("exit"))
    }

    fn enable_repl(&self) -> bool {
        true
    }

    fn on_subcommand(&mut self, name: &str, _: &clap::ArgMatches) {
        match name {
            "ping" => {
                println!("pong");
            }
            "exit" => {
                std::process::exit(0);
            }
            _ => {}
        }
    }

    fn format_prompt(&mut self) {
        print!("> ");
    }

    fn process_command(self, _: clap::Command) {}
}
