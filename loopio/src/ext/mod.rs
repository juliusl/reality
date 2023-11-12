use async_trait::async_trait;
use reality::prelude::*;
use tracing::trace;

use crate::host::HostCondition;
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

    /// Notify host w/ a condition,
    ///
    async fn notify_host(&self, host: &str, condition: &str) -> anyhow::Result<()>;
    
    /// Listen for a condition from the host,
    /// 
    async fn listen_host(&self, host: &str, condition: &str) -> Option<HostCondition>;
}

#[async_trait]
impl Ext for ThunkContext {
    async fn engine_handle(&self) -> Option<EngineHandle> {
        if let Some(handle) = self.node()
            .await
            .current_resource::<EngineHandle>(None) {
            if let Ok(handle) = handle.sync().await {
                Some(handle)
            } else {
                Some(handle)
            }
        } else {
            None
        }
    }

    async fn get_comments(&self) -> Option<Comments> {
        self.node()
            .await
            .current_resource(self.attribute.map(|a| a.transmute()))
    }

    async fn notify_host(&self, host: &str, condition: &str) -> anyhow::Result<()> {
        if let Some(host) = self.host(host).await {
            if let Some(host_condition) =
                host.current_resource::<HostCondition>(Some(ResourceKey::with_hash(condition)))
            {
                host_condition.notify();
            }
        }

        Ok(())
    }

    async fn listen_host(&self, host: &str, condition: &str) -> Option<HostCondition> {
        if let Some(host) = self.host(host).await {
            if let Some(host_condition) =
                host.current_resource::<HostCondition>(Some(ResourceKey::with_hash(condition)))
            {
                trace!("Found condition");
                return Some(host_condition);
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
        /// Adds a plugin to receive a signal from a host,
        /// 
        #[reality(ext)]
        receive_signal: super::ReceiveSignal,
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
}

#[derive(Reality, Default, Clone)]
#[reality(plugin, call = send_signal, rename = "send-signal", group = "loopio")]
pub struct SendSignal {
    #[reality(derive_fromstr)]
    name: String,
    host: String,
}

async fn send_signal(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let signal = tc.initialized::<SendSignal>().await;
    tc.notify_host(&signal.host, &signal.name).await
}

#[derive(Reality, Debug, Default, Clone)]
#[reality(plugin, call = receive_signal, rename = "receive-signal", group = "loopio")]
pub struct ReceiveSignal {
    #[reality(derive_fromstr)]
    name: String,
    host: String,
}

async fn receive_signal(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let signal = tc.initialized::<ReceiveSignal>().await;
    eprintln!("Listening for signal {:?}", signal);
    if let Some(listener) = tc.listen_host(&signal.host, &signal.name).await {
        listener.listen().await;
    }
    Ok(())
}
