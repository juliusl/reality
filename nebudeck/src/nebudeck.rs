use std::cell::OnceCell;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use loopio::engine::Engine;
use loopio::engine::EngineBuilder;
use loopio::engine::EngineHandle;
use loopio::foreground::ForegroundEngine;
use loopio::prelude::Dir;
use loopio::prelude::FrameUpdates;
use loopio::prelude::Package;
use loopio::prelude::Workspace;

use tracing::error;
use tracing::{debug, info};

use crate::base64::decode_field_packet;
use crate::desktop::Desktop;
use crate::desktop::DesktopApp;
use crate::ext::imgui_ext::ImguiMiddleware;
use crate::ext::RenderPipelineMiddleware;
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

        /// Initializes a directory,
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

        /// Initializes a runmd file,
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

        // Create .config/nbd/boot/nbd.md if missing
        init_runmd(&config_nbd_boot, "nbd.md", || {
            include_str!("../lib/runmd/nbd.md")
        })?;

        if !std::env::var(NBD_BOOT_ONLY).is_ok() {
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
        } else {
            info!("NBD_BOOT_ONLY is enabled, skipping project initialization");
        }

        // Configure the boot workspace
        let mut boot = Dir(config_nbd_boot).workspace();
        boot.set_name("nbd_boot");

        Ok(Self {
            boot,
            boot_package: OnceCell::new(),
            engine: OnceCell::new(),
            fg: OnceCell::new(),
        })
    }

    /// Boots nebudeck and opens dev tools window,
    ///
    pub fn open(self) -> anyhow::Result<()> {
        let mut nbd_boot = self.boot()?;

        let desktop = Desktop::new()?;

        let imgui_mw = ImguiMiddleware::new()
            .enable_imgui_demo_window()
            .enable_aux_demo_window()
            .middleware();

        let fg = nbd_boot.fg.take().unwrap();

        WgpuSystem::with(vec![imgui_mw]).delegate(desktop, fg)?;

        Ok(())
    }

    /// Boots nebudeck in cli mode,
    /// 
    pub fn start_cli(self) -> anyhow::Result<()> {
        self.start_cli_with(|e| e)
    }

    /// Boots nebudeck in cli mode w/ engine builder config,
    ///
    pub fn start_cli_with(self, config: impl Fn(EngineBuilder) -> EngineBuilder) -> anyhow::Result<()> {
        let mut booted = self.boot_with(config)?;

        let fg = booted.fg.take().unwrap();
        booted.delegate(Terminal, fg)?;
        Ok(())
    }

    /// Boots nebudeck
    ///
    fn boot(self) -> anyhow::Result<Self> {
        self.boot_with(|e| e)
    }

    /// Boots nebudeck with config
    /// 
    fn boot_with(self, config: impl Fn(EngineBuilder) -> EngineBuilder) -> anyhow::Result<Self> {
        let mut builder = config(Engine::builder());

        debug!("Building workspace {:#?}", self.boot);

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

/// Restrict init to initialized Rust packages,
///
/// **Note** If NBD_BOOT_ONLY is set, this check will be skipped
///
fn check_dir_is_rust_project(dir: impl Into<PathBuf>) {
    if std::env::var("NBD_BOOT_ONLY").is_ok() {
        info!("NBD_BOOT_ONLY is set, skipping rust project check");
        return;
    }

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

    fn process_command(&mut self, mut command: clap::Command) -> anyhow::Result<()> {
        debug!("Interpreting command {}", command.get_name());

        // Interpret an engine address from command
        let name = command.get_name().to_string();
        debug!("Found host `{}`", name);

        let matches = if let Ok(prog) = std::env::var(NBD_BOOT_PROG) {
            let prog = shlex::split(&prog).expect("should be valid cli arguments");
            info!("`NBD_PROG` env var is set, getting matches from {:?}", prog);
            command.clone().get_matches_from(prog)
        } else {
            command.clone().get_matches()
        };

        let mut frame_updates = FrameUpdates::default();

        // Resolve the engine address from subcommand settings
        let address = if let Some((group, matches)) = matches.subcommand() {
            debug!("Found group `{}`", group);
            
            if let Some((subcommand, matches)) = matches.subcommand() {
                // Collect frame updates from arg matches
                for arg in matches.ids().filter(|i| !i.as_str().starts_with("_")) {
                    if let Some(input) = matches.get_one::<String>(arg.as_str()) {
                        if let Some(fp) = matches.get_one::<String>(
                            format!("_{}_field_packet_enc", arg.as_str()).as_str(),
                        ) {
                            debug!("Found argument `{}`", arg);
                            if let Ok(decoded) = decode_field_packet(fp) {
                                let parse_op = decoded.parse(input.to_string());
                                frame_updates.frame.fields.push(parse_op);
                            }
                        }
                    }
                }

                // Format address
                if let Some(ext) = matches.get_one::<String>("_ext") {
                    debug!("Found ext type `{}`", ext);
                    format!("{group}/{subcommand}/{ext}")
                } else {
                    unreachable!()
                }
            } else {
                // If a subcommand is not set, check if the group has any subcommands
                if let Some(mut group) = command
                    .get_subcommands()
                    .find(|s| s.get_name() == group)
                    .cloned()
                {
                    if group.get_subcommands().next().is_some() {
                        error!("Missing subcommand");
                        group.print_help().ok();
                        return Err(anyhow!("Missing command group"));
                    } else {
                        // TODO: Configure Host and Sequence arguments
                        for arg in matches.ids().filter(|i| !i.as_str().starts_with("_")) {
                            if let Some(input) = matches.get_one::<String>(arg.as_str()) {
                                if let Some(fp) = matches.get_one::<String>(
                                    format!("_{}_field_packet_enc", arg.as_str()).as_str(),
                                ) {
                                    if let Ok(decoded) = decode_field_packet(fp) {
                                        debug!("Found argument `{}`", arg);
                                        let parse_op = decoded.parse(input.to_string());
                                        frame_updates.frame.fields.push(parse_op);
                                    }
                                }
                            }
                        }

                        // Group is actually a subcommand and also the address
                        group.get_name().to_string()
                    }
                } else {
                    unreachable!()
                }
            }
        } else {
            command.print_help().ok();
            return Err(anyhow!("Missing command group"));
        };

        debug!("Calling address `{}`", address);
        // TODO: If repl is enabled, move to `impl Nebudeck`
        if let Some(engine) = self.engine.get_mut() {
            if let Some(bg) = engine.background() {
                match bg.call(address) {
                    Ok(mut bgf) => {
                        // TODO: Add Progress Controller to stderr
                        if !frame_updates.frame.fields.is_empty() {
                            bgf.spawn_with_updates(frame_updates);
                        } else {
                            bgf.spawn();
                        }

                        // TODO: When Repl mode is added, skip this part
                        bgf.into_foreground().expect("should be able to call");
                    }
                    Err(err) => Err(anyhow!("Could not process command: {err}"))?,
                }
            }
        }

        Ok(())
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

    set_nbd_boot_prog("nbd_boot add-project terminal --label example");
    deck.start_cli_with(|e| e).expect("should be able to process command");
    ()
}

const NBD_BOOT_PROG: &'static str = "NBD_BOOT_PROG";

const NBD_BOOT_ONLY: &'static str = "NBD_BOOT_ONLY";

/// Sets NBD_BOOT_PROG w/ arguments to use w/ nbd_boot,
///
pub fn set_nbd_boot_prog(prog: impl AsRef<str>) {
    std::env::set_var(NBD_BOOT_PROG, prog.as_ref());
}

/// Enables NBD_BOOT_ONLY behavior,
///
pub fn set_nbd_boot_only() {
    std::env::set_var(NBD_BOOT_ONLY, "1");
}
