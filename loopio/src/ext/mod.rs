use async_trait::async_trait;
use reality::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::trace;

use self::wire_ext::VirtualBusExt;
use crate::engine::EngineHandle;

#[cfg(feature = "flexbuffers-ext")]
pub mod flexbuffers_ext;
#[cfg(feature = "hyper-ext")]
pub mod hyper_ext;
#[cfg(feature = "poem-ext")]
pub mod poem_ext;
#[cfg(feature = "std-ext")]
pub mod std_ext;
#[cfg(feature = "wire-ext")]
pub mod wire_ext;

/// General extensions for ThunkContext,
///
#[async_trait]
pub trait Ext {
    /// Returns an engine handle from storage,
    ///
    async fn engine_handle(&self) -> Option<EngineHandle>;
}

#[async_trait]
impl Ext for ThunkContext {
    /// Returns the current engine handle,
    ///
    #[inline]
    async fn engine_handle(&self) -> Option<EngineHandle> {
        self.node().await.root_ref().current()
    }
}

pub mod utility {
    use reality::prelude::*;

    /// Set of utilities built into the engine,
    ///
    #[derive(Reality, Clone, Default)]
    #[reality(ext, rename = "utility/loopio", call = noop, plugin)]
    pub struct Utility {
        /// Unused
        #[reality(derive_fromstr)]
        _unused: String,
        /// Adds plugins from std_ext::Stdio
        ///
        #[cfg(feature = "std-ext")]
        #[reality(plugin)]
        stdio: super::std_ext::Stdio,
        /// Adds a process plugin,
        ///
        #[cfg(feature = "std-ext")]
        #[reality(ext)]
        process: super::std_ext::Process,
        /// Adds an engine proxy plugin,
        ///
        #[cfg(feature = "poem-ext")]
        #[reality(ext)]
        engine_proxy: super::poem_ext::EngineProxy,
        /// Adds an reverse_proxy_config plugin,
        ///
        #[cfg(feature = "poem-ext")]
        #[reality(ext)]
        reverse_proxy_config: super::poem_ext::ReverseProxyConfig,
        /// Adds an reverse_proxy plugin,
        ///
        #[cfg(feature = "poem-ext")]
        #[reality(ext)]
        reverse_proxy: super::poem_ext::ReverseProxy,
        /// Adds a request plugin,
        ///
        #[cfg(feature = "hyper-ext")]
        #[reality(ext)]
        request: super::hyper_ext::Request,
    }

    async fn noop(_: &mut ThunkContext) -> anyhow::Result<()> {
        Ok(())
    }
}
