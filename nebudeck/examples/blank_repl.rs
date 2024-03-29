use loopio::engine::Engine;
use loopio::engine::EngineHandle;
use loopio::foreground::ForegroundEngine;
use loopio::prelude::Workspace;
use nebudeck::terminal::Terminal;
use nebudeck::terminal::TerminalApp;
use nebudeck::ControlBus;

/// Minimal example for starting a new terminal repl interaction,
///
fn main() {
    let mut engine = Engine::builder();

    let mut workspace = Workspace::new();
    workspace.add_local("lib/runmd/blank_repl.md");

    engine.set_workspace(workspace);

    let terminal = Terminal::default();

    BlankRepl
        .delegate(terminal, ForegroundEngine::new(engine))
        .unwrap();
}

#[derive(Default)]
struct BlankRepl;

impl ControlBus for BlankRepl {
    fn bind(&mut self, _: EngineHandle) {}
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
                None
            }
            "exit" => {
                std::process::exit(0);
            }
            _ => None,
        }
    }

    fn format_prompt(&mut self) {
        print!("> ");
    }

    fn process_command(&mut self, _: clap::Command) -> anyhow::Result<()> {
        Ok(())
    }
}
