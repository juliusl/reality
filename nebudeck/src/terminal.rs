use clap::ArgMatches;
use std::io::Write;
use std::ops::DerefMut;
use tracing::error;

use loopio::prelude::*;

use crate::controller::ControlBus;
use crate::BackgroundWork;
use crate::Controller;

/// Pointer-struct for providing an interaction loop,
///
#[derive(Default)]
pub struct Terminal;

impl<T: TerminalApp> Controller<T> for Terminal {
    fn take_control(self, app: Box<T>, engine: ForegroundEngine) -> BackgroundWork {
        let mut app: Box<dyn TerminalApp> = app;
        app.deref_mut().bind(engine.engine_handle());

        let cli = app.parse_command();

        if app.enable_repl() {
            loop {
                app.format_prompt();
                let _ = std::io::stdout().flush();

                let line = read_line();
                match line {
                    Ok(line) => {
                        let args = shlex::split(line.as_str()).unwrap_or_default();

                        match cli.clone().try_get_matches_from(args) {
                            Ok(matches) => match matches.subcommand() {
                                Some((subcommand, matches)) => {
                                    if let Some(mut _replacing) =
                                        app.on_subcommand(subcommand, matches)
                                    {
                                        _replacing.bind(engine.engine_handle());
                                        app = _replacing;
                                    }
                                }
                                None => {
                                    continue;
                                }
                            },
                            Err(err) => {
                                eprintln!("{err}");
                            }
                        }
                    }
                    Err(err) => {
                        error!("{err}");
                        std::process::exit(1);
                    }
                }
            }
        } else {
            app.process_command(cli);
        }

        None
    }
}

/// Reads a line from stdin returning the line buffer,
///
fn read_line() -> anyhow::Result<String> {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line)
}

/// Trait for interacting w/ a terminal interaction loop,
///
pub trait TerminalApp: ControlBus {
    /// Parses args returning and returns a command,
    ///
    fn parse_command(&mut self) -> clap::Command;

    /// Return true to enable REPL,
    ///
    fn enable_repl(&self) -> bool;

    /// Called before reading the next line,
    ///
    fn format_prompt(&mut self);

    /// Called on a subcommand,
    ///
    /// **Note** Relevant only when REPL is enabled
    ///
    fn on_subcommand(&mut self, name: &str, matches: &ArgMatches) -> Option<Box<dyn TerminalApp>>;

    /// Process a command,
    ///
    /// **Note**: Relevant only when REPL is disabled
    ///
    fn process_command(&mut self, command: clap::Command);
}
