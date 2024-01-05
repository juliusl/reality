use std::{path::{Path, PathBuf}, cell::OnceCell};

use clap::Arg;
use loopio::{prelude::{Dir, Package, Workspace}, engine::EngineHandle};
use tracing::{info, debug};

use crate::{ControlBus, terminal::TerminalApp, desktop::DesktopApp};

/// Entrypoint for interacting with a nebudeck project workspace,
///
pub struct Nebudeck {
    /// Boot workspace,
    /// 
    boot: Workspace,
    /// Nebudeck engine,
    /// 
    engine: OnceCell<EngineHandle>,
    /// Package state
    ///
    package: OnceCell<Package>,
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
        fn init_runmd(home_dir: &PathBuf, file: impl AsRef<Path>, init: fn() -> &'static str) -> anyhow::Result<PathBuf> {
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
        init_runmd(&home_dir, "run.md", || r#"
```runmd
# -- Nebudeck Project Manifest
# -- **Note** If no input is set, the name of the current directory will be used.
+ .project
 ```"#)?;
        
        // Create .config/nbd/boot/nbd.md if missing
        init_runmd(&config_nbd_boot, "nbd.md", || include_str!("../lib/runmd/nbd.md"))?;

        // Configure the boot workspace
        let boot = Dir(PathBuf::from(".config/nbd/boot/")).workspace();

        Ok(Self { boot, engine: OnceCell::new(), package: OnceCell::new() })
    }

    // /// Boots the nebudeck engine
    // /// 
    // fn boot(&self) -> anyhow::Result<Self> {
    //     todo!()
    // }

    // pub async fn engine(self) -> anyhow::Result<loopio::prelude::Engine> {
    //     todo!()
    // }

    // /// Returns a reference to the inner package,
    // ///
    // #[inline]
    // pub fn package_ref(&self) -> &Package {
    //     &self.package
    // }

    // /// Returns a mutable reference to the inner package,
    // ///
    // #[inline]
    // pub fn package_mut(&mut self) -> &mut Package {
    //     &mut self.package
    // }
}

fn check_dir_is_rust_project(dir: impl Into<PathBuf>) {
    let dir = dir.into();

    let cargo_toml = dir.join("Cargo.toml");

    let cargo_manifest =
        std::fs::read_to_string(cargo_toml).expect("should be able to read Cargo.toml");

    let table = toml::from_str::<toml::Table>(&cargo_manifest)
        .expect("should be able to parse toml of Cargo.toml");

    if table.get("workspace").filter(|w| w.is_table()).is_some() {
        panic!("Current directory must be a rust project, instead rust workspace found");
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
        // TODO: Need to get the resource key of the boot
        // TODO: Package -> clap::Command
        todo!()
    }

    fn process_command(&mut self, command: clap::Command) {
        debug!("Interpreting command {:?}", command);

        // Interpret an engine address from command
        let name = command.get_name().to_string();
        debug!("Found host {}", name);

        let matches = command.get_matches();
        if let Some(subcommand) = matches.subcommand_name() {
            debug!("Found ext path {}", subcommand);
            if let Some(ext) = matches.get_one::<&str>("ext") {
                debug!("Found ext type {}", ext);
                let address = format!("{name}/{subcommand}/{ext}");

                if let Some(engine) = self.engine.get_mut() {
                    if let Some(bg) = engine.background() {
                        match bg.call(address) {
                            Ok(mut bgf) => {
                                // TODO: Add Progress Controller to stderr
                                bgf.spawn();
                                debug!("Started background task");
                                bgf.into_foreground().expect("should be able to call");
                            },
                            Err(err) => panic!("Could not process command: {err}"),
                        }
                    }
                }
            }
        }
    }

    fn enable_repl(&self) -> bool { false }
    fn format_prompt(&mut self) {}
    fn on_subcommand(&mut self, _: &str, _: &clap::ArgMatches) -> Option<Box<dyn TerminalApp>> { None }
}

impl DesktopApp for Nebudeck {}

#[test]
#[tracing_test::traced_test]
fn test_init() {
    let deck = Nebudeck::init(std::env::current_dir().unwrap()).unwrap();

    eprintln!("{:?}", deck.boot);
}
