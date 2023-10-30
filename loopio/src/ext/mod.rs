use reality::{StorageTarget, ThunkContext};

use crate::engine::EngineHandle;

#[cfg(feature = "hyper-ext")]
pub mod hyper_ext;
#[cfg(feature = "poem-ext")]
pub mod poem_ext;
#[cfg(feature = "std-ext")]
pub mod std_ext;

/// General extensions for ThunkContext,
///
pub trait Ext {
    /// Returns an engine handle from storage,
    ///
    fn engine_handle(&self) -> Option<EngineHandle>;
}

impl Ext for ThunkContext {
    fn engine_handle(&self) -> Option<EngineHandle> {
        self.source
            .storage
            .try_read()
            .ok()
            .and_then(|s| s.resource::<EngineHandle>(None).map(|e| e.clone()))
    }
}

pub mod utility {
    use reality::prelude::*;

    /// Set of utilities built into the engine,
    /// 
    #[derive(Reality, Clone, Default)]
    #[reality(ext, rename="utility/loopio")]
    pub struct Utility {
        /// Unused
        #[reality(derive_fromstr)]
        _unused: String,
        /// Adds plugins from std_ext::Stdio
        /// 
        #[cfg(feature = "std-ext")]
        #[reality(plugin)]
        stdio: super::std_ext::Stdio,
        /// Adds an engine proxy plugin,
        /// 
        #[cfg(feature = "poem-ext")]
        #[reality(ext)]
        engine_proxy: super::poem_ext::EngineProxy,
        /// Adds a request plugin,
        /// 
        #[cfg(feature = "hyper-ext")]
        #[reality(ext)]
        request: super::hyper_ext::Request,
    }
}
