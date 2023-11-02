use async_trait::async_trait;
use reality::{Comments, StorageTarget, ThunkContext};

use crate::engine::EngineHandle;

#[cfg(feature = "hyper-ext")]
pub mod hyper_ext;
#[cfg(feature = "poem-ext")]
pub mod poem_ext;
#[cfg(feature = "std-ext")]
pub mod std_ext;
pub mod wire_ext;

/// General extensions for ThunkContext,
///
#[async_trait]
pub trait Ext {
    /// Returns an engine handle from storage,
    ///
    async fn engine_handle(&self) -> Option<EngineHandle>;

    /// Returns any comments added for this attribute,
    ///
    async fn get_comments(&self) -> Option<Comments>;
}

#[async_trait]
impl Ext for ThunkContext {
    async fn engine_handle(&self) -> Option<EngineHandle> {
        self.node().await.current_resource(None)
    }

    async fn get_comments(&self) -> Option<Comments> {
        self.node()
            .await
            .current_resource(self.attribute.map(|a| a.transmute()))
    }
}

pub mod utility {
    use reality::prelude::*;

    /// Set of utilities built into the engine,
    ///
    #[derive(Reality, Clone, Default)]
    #[reality(ext, rename = "utility/loopio")]
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
