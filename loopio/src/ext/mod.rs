use async_trait::async_trait;
use reality::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::engine::EngineHandle;
use crate::prelude::{Action, Address, Host};

use self::wire_ext::{VirtualBus, VirtualBusExt};

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

    /// Notify host w/ a condition,
    ///
    async fn notify_host(&self, host: &str) -> anyhow::Result<()>;

    /// Listen for a condition from the host,
    ///
    async fn listen_host(&self, host: &str) -> Option<VirtualBus>;

    /// If the decoration property "notify" is defined, notifies host on the condition
    /// named there.
    ///
    async fn on_notify_host(&self, host: &str) -> anyhow::Result<()>;
}

#[async_trait]
impl Ext for ThunkContext {
    /// Returns the current engine handle,
    ///
    async fn engine_handle(&self) -> Option<EngineHandle> {
        if let Some(handle) = self
            .node()
            .await
            .current_resource::<EngineHandle>(ResourceKey::root())
        {
            if let Ok(handle) = handle.sync().await {
                Some(handle)
            } else {
                Some(handle)
            }
        } else {
            None
        }
    }

    async fn on_notify_host(&self, host: &str) -> anyhow::Result<()> {
        if let Some(_notify) = self.property("notify") {
            self.notify_host(host).await?;
        }
        Ok(())
    }

    async fn notify_host(&self, _host: &str) -> anyhow::Result<()> {
        // if let Some(host) = self.host(host).await {
        //     if let Some(host_condition) =
        //         host.current_resource::<HostCondition>(Some(ResourceKey::with_hash(condition)))
        //     {
        //         host_condition.notify();
        //     }
        // }

        Ok(())
    }

    /// Searches for a virtual bus hosted by this context,
    ///
    async fn listen_host(&self, host: &str) -> Option<VirtualBus> {
        if let Some(eh) = self.engine_handle().await {
            if let Ok(host) = eh.hosted_resource(format!("{host}://")).await {
                let host = host.context().initialized::<Host>().await;
                // TODO
                /*
                    Host should be an entry point to different attribute streams
                */
                let address = Address::new(host.address());
                let vbus = host.context().virtual_bus(address).await;

                return Some(vbus);
            }
        }
        None
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
