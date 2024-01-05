use std::cell::OnceCell;
use std::path::Path;
use std::path::PathBuf;

use loopio::engine::Engine;
use loopio::engine::EngineHandle;
use loopio::foreground::ForegroundEngine;
use loopio::prelude::Dir;
use loopio::prelude::Package;
use loopio::prelude::Workspace;

use tracing::error;
use tracing::{debug, info};

use crate::desktop::Desktop;
use crate::desktop::DesktopApp;
use crate::ext::WgpuSystem;
use crate::terminal::Terminal;
use crate::terminal::TerminalApp;
use crate::ControlBus;

/// Entrypoint for interacting with a nebudeck project workspace,
///
pub struct Nebudeck {
    /// Boot workspace,
    ///
    boot: Workspace,
    /// Nebudeck engine,
    ///
    engine: OnceCell<EngineHandle>,
    /// Boot package state,
    ///
    boot_package: OnceCell<Package>,
    /// Foreground engine,
    ///
    fg: OnceCell<ForegroundEngine>,
}

impl Nebudeck {
    /// Initializes a directory for Nebudeck,
    ///
    pub fn init(home_dir: PathBuf) -> anyhow::Result<Self> {
        // The home directory being initialized must already exist and be available
        let home_dir = home_dir.canonicalize()?;

        // Confirm directory is a rust project
        check_dir_is_rust_project(home_dir.clone());

        /// Initialize a directory,
        ///
        fn init_dir(home_dir: &PathBuf, dir: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
            let dir = home_dir.join(dir.as_ref());
            if !dir.exists() {
                info!("Creating {:?} directory", dir);
                std::fs::create_dir_all(&dir)?;
            } else {
                info!("Found {:?}", dir);
            }

            Ok(dir)
        }

        /// Initialize a runmd file,
        ///
        fn init_runmd(
            home_dir: &PathBuf,
            file: impl AsRef<Path>,
            init: fn() -> &'static str,
        ) -> anyhow::Result<PathBuf> {
            let file = home_dir.join(file.as_ref());
            if !file.exists() {
                info!("Creating {:?} file", file);

                std::fs::write(&file, init())?;
            } else {
                info!("Found {:?}", file);
            }

            Ok(file)
        }

        // Create .config/nbd/ if missing
        let _ = init_dir(&home_dir, ".config/nbd/")?;

        // Create .config/nbd/boot/ if missing
        let config_nbd_boot = init_dir(&home_dir, ".config/nbd/boot/")?;

        // Create lib/runmd if missing
        let _ = init_dir(&home_dir, "lib/runmd/")?;

        // Create Run.md manifest if missing
        init_runmd(&home_dir, "run.md", || {
            r#"
```runmd
# -- Nebudeck Project Manifest
# -- **Note** If no input is set, the name of the current directory will be used.
+ .project
 ```"#
        })?;

        // Create .config/nbd/boot/nbd.md if missing
        init_runmd(&config_nbd_boot, "nbd.md", || {
            include_str!("../lib/runmd/nbd.md")
        })?;

        // Configure the boot workspace
        let mut boot = Dir(config_nbd_boot).workspace();
        boot.set_name("nbd_boot");

        Ok(Self {
            boot,
            engine: OnceCell::new(),
            boot_package: OnceCell::new(),
            fg: OnceCell::new(),
        })
    }

    /// Boots nebudeck and opens dev tools window,
    ///
    pub fn open(self) -> anyhow::Result<()> {
        let mut booted = self.boot()?;

        let desktop = Desktop::new()?;

        WgpuSystem::new().delegate(desktop, booted.fg.take().unwrap());

        Ok(())
    }

    /// Boots nebudeck in cli mode,
    ///
    pub fn start_cli(self) -> anyhow::Result<()> {
        let mut booted = self.boot()?;

        let fg = booted.fg.take().unwrap();
        booted.delegate(Terminal, fg);
        Ok(())
    }

    /// Boots nebudeck
    ///
    fn boot(self) -> anyhow::Result<Self> {
        let mut builder = Engine::builder();
        builder.set_workspace(self.boot.clone());

        let foreground = ForegroundEngine::new(builder);

        assert!(
            self.boot_package.set(foreground.package.clone()).is_ok(),
            "should only boot once"
        );
        assert!(self.fg.set(foreground).is_ok(), "should only boot once");

        Ok(self)
    }
}

fn check_dir_is_rust_project(dir: impl Into<PathBuf>) {
    let dir = dir.into();

    let cargo_toml = dir.join("Cargo.toml");

    let cargo_manifest = std::fs::read_to_string(cargo_toml)
        .expect("Target directory must be a Rust package. Cargo.toml not found.");

    let table = toml::from_str::<toml::Table>(&cargo_manifest)
        .expect("should be able to parse toml of Cargo.toml");

    if table.get("workspace").filter(|w| w.is_table()).is_some() {
        panic!("Target directory must be a Rust package. Rust workspace detected.");
    }

    info!("Current directory is a Rust Project");
}

impl ControlBus for Nebudeck {
    fn bind(&mut self, eh: loopio::prelude::EngineHandle) {
        self.engine.set(eh).expect("should be able to set");
    }
}

impl TerminalApp for Nebudeck {
    fn parse_command(&mut self) -> clap::Command {
        let boot = self.boot_package.take().expect("should be compiled");
        let boot: clap::Command = boot.into();
        // TODO: Enable Repl mode?
        boot
    }

    fn process_command(&mut self, mut command: clap::Command) {
        debug!("Interpreting command {:#?}", command);

        // Interpret an engine address from command
        let name = command.get_name().to_string();
        debug!("Found host `{}`", name);

        let matches = if let Ok(prog) = std::env::var("NBD_PROG") {
            let prog = shlex::split(&prog).expect("should be valid cli arguments");
            info!("`NBD_PROG` env var is set, getting matches from {:?}", prog);
            command.clone().get_matches_from(prog)
        } else {
            command.clone().get_matches()
        };

        if let Some((group, matches)) = matches.subcommand() {
            debug!("Found group `{}`", group);

            if let Some((subcommand, matches)) = matches.subcommand() {
                if let Some(ext) = matches.get_one::<&str>("ext") {
                    debug!("Found ext type `{}`", ext);
                    let address = format!("{name}/{subcommand}/{ext}");

                    if let Some(engine) = self.engine.get_mut() {
                        if let Some(bg) = engine.background() {
                            match bg.call(address) {
                                Ok(mut bgf) => {
                                    // TODO: Add Progress Controller to stderr
                                    bgf.spawn();
                                    debug!("Started background task");
                                    bgf.into_foreground().expect("should be able to call");
                                }
                                Err(err) => panic!("Could not process command: {err}"),
                            }
                        }
                    }
                }
            } else {
                error!("Missing subcommand");
                if let Some(mut group) = command
                    .get_subcommands()
                    .find(|s| s.get_name() == group)
                    .cloned()
                {
                    group.print_help().ok();
                }
            }
        } else {
            error!("Missing command group");
            command.print_help().ok();
        }
    }

    fn enable_repl(&self) -> bool {
        false
    }
    fn format_prompt(&mut self) {}
    fn on_subcommand(&mut self, _: &str, _: &clap::ArgMatches) -> Option<Box<dyn TerminalApp>> {
        None
    }
}

impl DesktopApp for Nebudeck {}

#[test]
#[tracing_test::traced_test]
fn test_init() {
    let tmp = std::env::temp_dir().join("test_init");
    if tmp.exists() {
        eprintln!("Removing old directory");
        std::fs::remove_dir_all(&tmp).unwrap()
    }
    std::fs::create_dir_all(&tmp).unwrap();
    let cargo = tmp.join("Cargo.toml");
    std::fs::write(cargo, "[package]").unwrap();

    let deck = Nebudeck::init(tmp).unwrap();
    eprintln!("{:?}", deck.boot);

    std::env::set_var("NBD_PROG", "nbd_boot install");
    let _ = deck.start_cli().unwrap();

    ()
}
