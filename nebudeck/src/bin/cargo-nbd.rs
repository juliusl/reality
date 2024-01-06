use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use nebudeck::set_nbd_boot_only;
use nebudeck::set_nbd_boot_prog;
use nebudeck::Nebudeck;

use loopio::prelude::*;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Cargo plugin for nbd dev/boot tools,
///
fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let args = std::env::args()
        .take_while(|a| a != "--")
        .collect::<Vec<_>>();

    // If called as a cargo-plugin, CargoWrapper needs to be used as the parser
    let cli = if matches!(std::env::args().next(), Some(ref arg) if arg.ends_with("cargo-nbd")) {
        let wrapper = CargoWrapper::parse_from(args);
        match wrapper {
            CargoWrapper::Nbd(cli) => cli,
        }
    } else {
        // If called as cargo-nbd
        Cli::parse_from(args)
    };

    match cli.command {
        Commands::Init { dir } => {
            let _ = Nebudeck::init(
                dir.clone()
                    .or(cli.home)
                    .unwrap_or_else(|| std::env::current_dir().unwrap()),
            )?;
        }
        Commands::Build { dir } => {
            let deck = Nebudeck::init(
                dir.clone()
                    .or(cli.home)
                    .unwrap_or_else(|| std::env::current_dir().unwrap()),
            )?;

            set_nbd_boot_prog("nbd_boot build");
            deck.start_cli()?;
        }
        Commands::Add { dir, project_type } => {
            let project_args = match project_type {
                ProjectTypes::Terminal {} => "terminal",
                ProjectTypes::Desktop { .. } => "desktop",
            };

            let dir = dir
                .clone()
                .or(cli.home)
                .unwrap_or_else(|| std::env::current_dir().unwrap());

            let deck = Nebudeck::init(dir)?;

            let rest = std::env::args()
                .skip_while(|a| a != "--")
                .skip(1)
                .collect::<Vec<_>>();

            let args = shlex::join(rest[..].iter().map(|r| r.as_str()));

            set_nbd_boot_prog(format!("nbd_boot add-project {project_args} {args}"));
            deck.start_cli_with(|mut e| {
                e.enable::<ProjectTypes>();
                e
            })?;
        }
        Commands::Run => {
            // Only initialzies .config/nbd if not already initialized, skips rust project check
            set_nbd_boot_only();
            let _deck =
                Nebudeck::init(cli.home.unwrap_or_else(|| std::env::current_dir().unwrap()))?;

            // TODO -- Build and Run?
            // set_nbd_boot_prog("nbd_boot build");
            // deck.start_cli()?;
        }
    }

    Ok(())
}

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum CargoWrapper {
    Nbd(Cli),
}

#[derive(Parser)]
#[command(name = "cargo-nbd")]
#[command(bin_name = "cargo-nbd")]
struct Cli {
    /// Path to directory to use as NBD_HOME,
    ///
    #[arg(long, env("NBD_HOME"))]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

/// Enumeration of subcommands,
///
#[derive(Subcommand)]
enum Commands {
    /// Initializes a directory for w/ nebudeck project files and boot config.
    Init {
        /// Target directory to initialize, overrides NBD_HOME.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Appends a new project item to NBD_HOME/run.md,
    ///
    /// Creates a new file under lib/runmd/<PROJECT-NAME>.md.
    ///
    /// **Note** If a project file already exists, this command will result in an error.
    ///
    Add {
        /// Target directory to add a project to, overrides NBD_HOME.
        #[arg(long)]
        dir: Option<PathBuf>,
        #[command(subcommand)]
        project_type: ProjectTypes,
    },
    /// Builds engines w/ projects specified by NBD_HOME/run.md.
    Build {
        /// Target directory to build, overrides NBD_HOME.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Runs the engine in the current context, sets NBD_BOOT_ONLY implicitly.
    Run,
}
/// Group of project types that can be added w/ cargo-nbd
///
#[derive(Reality, Default, Clone, Subcommand)]
#[plugin_def(
    call = todo
)]
#[parse_def(rename = "project")]
enum ProjectTypes {
    /// Terminal app project,
    ///
    #[default]
    Terminal,
    /// Desktop app project,
    ///
    Desktop {
        /// Enables a wgpu-based desktop app
        ///
        #[arg(action)]
        #[reality(ffi=bool, wire=into_box_from_wire)]
        wgpu: bool,
        /// Enables a wgpu-based desktop app w/ imgui middleware. Implicitly sets --wgpu.
        ///
        #[arg(action)]
        #[reality(ffi=bool, wire=into_box_from_wire)]
        wgpu_imgui: bool,
    },
}

async fn todo(_: &mut ThunkContext) -> anyhow::Result<()> {
    Ok(())
}

impl FromStr for ProjectTypes {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "terminal" => Ok(Self::Terminal),
            "desktop" => Ok(Self::Desktop {
                wgpu: false,
                wgpu_imgui: false,
            }),
            _ => Err(anyhow!("Unknown project type")),
        }
    }
}

#[test]
fn test_main() -> anyhow::Result<()> {
    let tmp = std::env::temp_dir().join("test_init");
    if tmp.exists() {
        eprintln!("Removing old directory");
        std::fs::remove_dir_all(&tmp).unwrap()
    }
    std::fs::create_dir_all(&tmp).unwrap();
    let cargo = tmp.join("Cargo.toml");
    std::fs::write(cargo, "[package]").unwrap();

    let deck = Nebudeck::init(tmp)?;

    set_nbd_boot_prog(format!("nbd_boot add-project terminal --help"));
    deck.start_cli_with(|mut e| {
        e.enable::<ProjectTypes>();
        e
    })?;

    Ok(())
}
