use std::io::Write;

use clap::ArgMatches;
use tracing::error;

use crate::project_loop::InteractionLoop;
use crate::project_loop::AppType;

/// Pointer-struct for providing an interaction loop,
/// 
pub struct Terminal;

impl<T: TerminalApp> InteractionLoop<T> for Terminal {
    fn take_control<S: reality::StorageTarget>(
        self,
        project_loop: crate::project_loop::ProjectLoop<S>,
    ) {
        let mut app = T::create(project_loop);

        let cli = app.parse_command();

        if app.enable_repl() {
            loop {
                app.format_prompt();
                let _ = std::io::stdout().flush();

                let line = read_line();
                match line {
                    Ok(line) => {
                        let args = shlex::split(line.as_str()).unwrap_or_default();

                        let matches = cli.clone().try_get_matches_from(args).unwrap();

                        match matches.subcommand() {
                            Some((subcommand, matches)) => app.on_subcommand(subcommand, matches),
                            None => {
                                continue;
                            },
                        }
                    }
                    Err(err) => {
                        error!("{err}");
                    }
                }
            }
        } else {
            app.process_command(cli);
        }
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
pub trait TerminalApp: AppType {
    /// Parses args returning and returns a command,
    ///
    fn parse_command(&mut self) -> clap::Command;

    /// Return true to enable REPL,
    ///
    fn enable_repl(&self) -> bool;

    /// Called before reading the next line,
    /// 
    fn format_prompt(&mut self);

    /// Process a command,
    /// 
    /// **Note**: Relevant only when REPL is disabled
    /// 
    fn process_command(self, command: clap::Command);

    /// Called on a subcommand,
    ///
    /// **Note** Relevant only when REPL is enabled
    ///
    fn on_subcommand(&mut self, name: &str, matches: &ArgMatches);
}
