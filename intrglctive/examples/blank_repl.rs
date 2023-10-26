use intrglctive::{
    project_loop::{AppType, ProjectLoop},
    terminal::{Terminal, TerminalApp},
};
use reality::{Project, Shared};

/// Minimal example for starting a new terminal repl interaction,
/// 
fn main() {
    BlankRepl::start_interaction(
        ProjectLoop::new(Project::new(Shared::default())), 
        Terminal
    );
}

struct BlankRepl;

impl AppType for BlankRepl {
    fn create<S: reality::StorageTarget + 'static>(
        _: intrglctive::project_loop::ProjectLoop<S>,
    ) -> Self {
        BlankRepl
    }
}

impl TerminalApp for BlankRepl {
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
