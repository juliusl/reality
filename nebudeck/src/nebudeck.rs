use std::path::PathBuf;

use loopio::prelude::Program;

/// Entrypoint for interacting with a nebudeck project workspace,
/// 
#[derive(Default, Clone)]
pub struct Nebudeck {
    /// Collection of compiled nebudeck projects,
    /// 
    projects: Vec<Program>
}

impl Nebudeck {
    /// Initializes a directory for nebudeck,
    /// 
    pub fn init(home_dir: PathBuf) -> anyhow::Result<Self> {
        // The home directory being initialized must already exist and be available
        let home_dir = home_dir.canonicalize()?;
        todo!()
    }

    pub async fn engine(self) -> anyhow::Result<loopio::prelude::Engine> {
        todo!()
    }
}