use std::cell::OnceCell;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use clap::Subcommand;
use loopio::prelude::*;

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
        self.start_cli_with(Engine::builder())
    }

    /// Boots nebudeck in cli mode w/ engine builder config,
    ///
    pub fn start_cli_with(self, engine_builder: EngineBuilder) -> anyhow::Result<()> {
        let mut booted = self.boot_with(engine_builder)?;

        let fg = booted.fg.take().unwrap();
        booted.delegate(Terminal, fg)?;
        Ok(())
    }

    /// Boots nebudeck
    ///
    fn boot(self) -> anyhow::Result<Self> {
        self.boot_with(Engine::builder())
    }

    /// Boots nebudeck with engine builder
    ///
    fn boot_with(self, mut engine_builder: EngineBuilder) -> anyhow::Result<Self> {
        engine_builder.enable::<ProjectTypes>();

        debug!("Building workspace {:#?}", self.boot);

        engine_builder.set_workspace(self.boot.clone());

        let foreground = ForegroundEngine::new(engine_builder);

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
                for arg in matches
                    .ids()
                    .filter(|i| !i.as_str().starts_with("internal"))
                {
                    if let Some(input) = matches.get_one::<String>(arg.as_str()) {
                        if let Ok(Some(fp)) = matches.try_get_one::<String>(
                            format!("internal_{}_field_packet_enc", arg.as_str()).as_str(),
                        ) {
                            debug!("Found argument `{}`", arg);
                            if let Ok(decoded) = decode_field_packet(fp) {
                                let parse_op = decoded.parse(input.to_string());
                                frame_updates.frame.fields.push(parse_op);
                            }
                        } else {
                            frame_updates.set_property(arg.as_str(), input);
                        }
                    }
                }

                // Format address
                if let Some(ext) = matches.get_one::<String>("internal_ext") {
                    debug!("Found ext type `{}`", ext);
                    format!("{group}/{subcommand}/{ext}")
                } else {
                    unreachable!()
                }
            } else if let Some(mut group) = command
                .get_subcommands()
                .find(|s| s.get_name() == group)
                .cloned()
            {
                // If a subcommand is not set, check if the group has any subcommands
                if group.get_subcommands().next().is_some() {
                    error!("Missing subcommand");
                    group.print_help().ok();
                    return Err(anyhow!("Missing command group"));
                } else {
                    // TODO: Configure Host and Sequence arguments
                    for arg in matches
                        .ids()
                        .filter(|i| !i.as_str().starts_with("internal"))
                    {
                        if let Some(input) = matches.get_one::<String>(arg.as_str()) {
                            if let Ok(Some(fp)) = matches.try_get_one::<String>(
                                format!("internal_{}_field_packet_enc", arg.as_str()).as_str(),
                            ) {
                                if let Ok(decoded) = decode_field_packet(fp) {
                                    debug!("Found argument `{}`", arg);
                                    let parse_op = decoded.parse(input.to_string());
                                    frame_updates.frame.fields.push(parse_op);
                                }
                            } else {
                                frame_updates.set_property(arg.as_str(), input);
                            }
                        }
                    }

                    // Group is actually a subcommand and also the address
                    group.get_name().to_string()
                }
            } else {
                unreachable!()
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
                        if frame_updates.has_update() {
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

    set_nbd_boot_prog("nbd_boot add-project terminal");
    deck.start_cli().expect("should be able to process command");

    // Compiler thread
    std::thread::Builder::new().name("compile".to_string()).spawn(|| {
        let _runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();

        loopio::prelude::runir::prelude::set_entropy();

        let local_set = tokio::task::LocalSet::new();
        local_set.spawn_local(async {

        });

        

    }).unwrap();

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

/// Group of project types that can be added w/ cargo-nbd
///
#[derive(Reality, Debug, Default, Clone, Subcommand)]
#[plugin_def(
    call = create_project
)]
#[parse_def(rename = "project")]
pub enum ProjectTypes {
    /// Terminal app project,
    ///
    #[default]
    Terminal,
    /// Desktop app project,
    ///
    Desktop {
        /// Title of the window
        ///
        #[arg(long, default_value = "Desktop App")]
        #[reality(ffi)]
        title: String,
        /// Height of window
        ///
        #[arg(long, default_value = "1080.0")]
        #[reality(ffi)]
        height: f32,
        /// Width of window
        ///
        #[arg(long, default_value = "1920.0")]
        #[reality(ffi)]
        width: f32,
    },
}

/// Create project files,
///
async fn create_project(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.as_remote_plugin::<ProjectTypes>().await;

    if let Some(name) = tc.property("name") {
        eprintln!("Creating project {name}");
    }

    match init {
        ProjectTypes::Terminal => {}
        ProjectTypes::Desktop {
            title,
            height,
            width,
        } => {
            eprintln!("{title} {height} x {width}");
        }
    }

    Ok(())
}

impl FromStr for ProjectTypes {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "terminal" => Ok(Self::Terminal),
            "desktop" => Ok(Self::Desktop {
                title: String::new(),
                height: 0.0,
                width: 0.0,
            }),
            _ => Err(anyhow!("Unknown project type")),
        }
    }
}
