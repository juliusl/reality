use loopio::engine::Engine;
use loopio::engine::EngineHandle;
use nebudeck::ControlBus;
use nebudeck::terminal::TerminalApp;
use nebudeck::terminal::Terminal;
/// Minimal example for starting a new terminal repl interaction,
/// 
fn main() {
    BlankRepl.delegate(
        Terminal::default(),
        Engine::new(),
    );
}

#[derive(Default)]
struct BlankRepl;

impl ControlBus for BlankRepl {
    fn bind(&mut self, _: EngineHandle) {
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

    fn on_subcommand(&mut self, name: &str, _: &clap::ArgMatches) -> Option<Box<dyn TerminalApp>> {
        match name {
            "ping" => {
                println!("pong");
                todo!()
            }
            "exit" => {
                std::process::exit(0);
            }
            _ => {
                todo!()
            }
        }
    }

    fn format_prompt(&mut self) {
        print!("> ");
    }

    fn process_command(&mut self, _: clap::Command) {}
}
