use reality::v2::prelude::*;

use crate::plugin::Plugin;

/// Component that prints messages to stderr and stdout,
///
#[derive(Runmd, Debug, Clone, Component)]
#[storage(specs::VecStorage)]
#[compile(ThunkCall)]
pub struct Println {
    /// Input to the println component,
    println: String,
    /// Map of properties that can be used to format lines being printed,
    #[config(ext=plugin.map)]
    fmt: Vec<String>,
    /// Lines to print to stderr,
    #[config(ext=plugin.format)]
    stderr: Vec<String>,
    /// Lines to print to stdout,
    #[config(ext=plugin.format)]
    stdout: Vec<String>,
    /// Plugin extension
    #[ext]
    plugin: Plugin,
}

#[async_trait]
impl reality::v2::Call for Println {
    async fn call(&self) -> Result<Properties> {
        for out in self.stdout.iter() {
            self.plugin
                .apply_formatting("fmt", "stdout", out)
                .map_or_else(
                    || {
                        println!("{out}");
                    },
                    |f| {
                        println!("{f}");
                    },
                );
        }

        for err in self.stderr.iter() {
            self.plugin
                .apply_formatting("fmt", "stderr", err)
                .map_or_else(
                    || {
                        eprintln!("{err}");
                    },
                    |f| {
                        eprintln!("{f}");
                    },
                );
        }

        Err(Error::skip())
    }
}

impl Println {
    /// Returns a new empty Println component,
    ///
    pub const fn new() -> Self {
        Self {
            println: String::new(),
            stderr: vec![],
            stdout: vec![],
            fmt: vec![],
            plugin: Plugin::new(),
        }
    }
}
