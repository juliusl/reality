use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;
use nebudeck::set_nbd_boot_only;
use nebudeck::set_nbd_boot_prog;
use nebudeck::Nebudeck;
use nebudeck::ProjectTypes;

use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Cargo plugin for nbd dev/boot tools,
///
fn main() -> anyhow::Result<()> {
    // Set up logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    // Only parse from args before "--"
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
        Commands::Add {
            dir,
            project_type,
            name,
        } => {
            let project_args = match project_type {
                ProjectTypes::Terminal {} => "terminal",
                ProjectTypes::Desktop { .. } => "desktop",
            };

            // NBD_HOME directory
            let home_dir = dir
                .clone()
                .or(cli.home)
                .unwrap_or_else(|| std::env::current_dir().unwrap());

            let deck = Nebudeck::init(home_dir)?;

            // Pass in args after "--"
            let rest = std::env::args()
                .skip_while(|a| a != "--")
                .skip(1)
                .collect::<Vec<_>>();

            let mut args = shlex::join(rest[..].iter().map(|r| r.as_str()));

            if !args.contains("--name") {
                args = format!("{args} --name {name}");
            }

            set_nbd_boot_prog(format!("nbd_boot add-project {project_args} {args}"));
            deck.start_cli()?;
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
    /// Adds a new starter project to env.
    ///
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
        /// Project type to add.
        #[command(subcommand)]
        project_type: ProjectTypes,
        /// Name of the project to add.
        #[arg(long, default_value = "new_project")]
        name: String,
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
