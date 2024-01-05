use std::path::PathBuf;

use loopio::prelude::Package;
use tracing::info;

/// Entrypoint for interacting with a nebudeck project workspace,
///
pub struct Nebudeck {
    /// Package state
    ///
    package: Package,
}

impl Nebudeck {
    /// Initializes a directory for nebudeck,
    ///
    pub fn init(home_dir: PathBuf) -> anyhow::Result<Self> {
        // The home directory being initialized must already exist and be available
        let home_dir = home_dir.canonicalize()?;

        let lib_runmd = home_dir.join("lib/runmd/");

        // Create 
        if !lib_runmd.exists() {
            info!("Creating lib/runmd directory");
            std::fs::create_dir_all(lib_runmd)?;
        } else {
            info!("Found lib/runmd");
        }


        todo!()
    }

    pub async fn engine(self) -> anyhow::Result<loopio::prelude::Engine> {
        todo!()
    }

    /// Returns a reference to the inner package,
    ///
    #[inline]
    pub fn package_ref(&self) -> &Package {
        &self.package
    }

    /// Returns a mutable reference to the inner package,
    ///
    #[inline]
    pub fn package_mut(&mut self) -> &mut Package {
        &mut self.package
    }
}

// /// Pointer-struct representing the home directory,
// ///
// struct HomeDir;

// impl HomeDir {
// }
