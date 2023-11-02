use std::collections::BTreeMap;

use futures_util::Future;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::trace;
use uuid::Uuid;

use crate::prelude::Latest;
use crate::AsyncStorageTarget;
use crate::Attribute;
use crate::BlockObject;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;

use super::prelude::*;

/// Struct containing shared context between plugins,
///
pub struct Context {
    /// Map of host storages this context can access,
    ///
    pub hosts: BTreeMap<String, AsyncStorageTarget<Shared>>,
    /// Node storage mapping to this context,
    ///
    pub node: AsyncStorageTarget<Shared>,
    /// Transient storage target,
    ///
    pub transient: AsyncStorageTarget<Shared>,
    /// Cancellation token that can be used by the engine to signal shutdown,
    ///
    pub cancellation: tokio_util::sync::CancellationToken,
    /// Attribute for this context,
    ///
    pub attribute: Option<ResourceKey<Attribute>>,
    /// If the context has been branched, this will be the Uuid assigned to the variant,
    ///
    pub variant_id: Option<Uuid>,
}

impl From<AsyncStorageTarget<Shared>> for Context {
    fn from(value: AsyncStorageTarget<Shared>) -> Self {
        let handle = value.runtime.clone().expect("should have a runtime");
        Self {
            hosts: BTreeMap::new(),
            node: value,
            attribute: None,
            transient: Shared::default().into_thread_safe_with(handle),
            cancellation: CancellationToken::new(),
            variant_id: None,
        }
    }
}

impl Clone for Context {
    fn clone(&self) -> Self {
        Self {
            hosts: self.hosts.clone(),
            node: self.node.clone(),
            attribute: self.attribute,
            transient: self.transient.clone(),
            cancellation: self.cancellation.clone(),
            variant_id: None,
        }
    }
}

impl Context {
    /// Creates a branched thunk context,
    ///
    pub fn branch(&self) -> (Uuid, Self) {
        let mut next = self.clone();
        // Create a variant for the type created here
        let variant_id = uuid::Uuid::new_v4();
        if let Some(attr) = next.attribute.as_mut() {
            *attr = attr.branch(variant_id);
        }
        next.variant_id = Some(variant_id);
        (variant_id, next)
    }
    /// Reset the transient storage,
    ///
    pub fn reset(&mut self) {
        let handle = self.node.runtime.clone().expect("should have a runtime");
        self.transient = Shared::default().into_thread_safe_with(handle);
    }

    /// Calls the thunk fn related to this context,
    ///
    pub async fn call(&self) -> anyhow::Result<Option<Context>> {
        let thunk = self
            .node()
            .await
            .current_resource::<ThunkFn>(self.attribute.map(|a| a.transmute()));
        if let Some(thunk) = thunk {
            (thunk)(self.clone()).await
        } else {
            Err(anyhow::anyhow!("Did not execute thunk"))
        }
    }

    /// Sets the attribute for this context,
    ///
    pub fn set_attribute(&mut self, attribute: ResourceKey<Attribute>) {
        self.attribute = Some(attribute);
    }

    /// Get read access to source storage,
    ///
    pub async fn node(&self) -> Shared {
        self.node.storage.latest().await
    }

    /// Get mutable access to host storage,
    ///
    /// # Safety
    ///
    /// Marked unsafe because will mutate the host storage. Host storage is shared by all contexts associated to a specific host.
    ///
    pub async unsafe fn host_mut(
        &self,
        name: impl AsRef<str>,
    ) -> Option<tokio::sync::RwLockWriteGuard<Shared>> {
        println!("Looking for {} in {:?}", name.as_ref(), self.hosts.keys());
        if let Some(host) = self.hosts.get(name.as_ref()) {
            Some(host.storage.write().await)
        } else {
            None
        }
    }

    /// Get read access to host storage,
    ///
    pub async fn host(&self, name: impl AsRef<str>) -> Option<Shared> {
        trace!("Looking for {} in {:?}", name.as_ref(), self.hosts.keys());
        if let Some(host) = self.hosts.get(name.as_ref()) {
            Some(host.storage.latest().await)
        } else {
            None
        }
    }

    /// Get mutable access to source storage,
    ///
    /// # Safety
    ///
    /// Marked unsafe because will mutate the source storage. Source storage is re-used on each execution.
    ///
    pub async unsafe fn node_mut(&self) -> tokio::sync::RwLockWriteGuard<Shared> {
        self.node.storage.write().await
    }

    /// (unsafe) Tries to get mutable access to source storage,
    ///
    /// # Safety
    ///
    /// Marked unsafe because will mutate the source storage. Source storage is re-used on each execution.
    ///
    pub unsafe fn try_source_mut(&mut self) -> Option<tokio::sync::RwLockWriteGuard<Shared>> {
        self.node.storage.try_write().ok()
    }

    /// Returns the transient storage target,
    ///
    /// **Note**: During an operation run dispatch queues are drained before each thunk execution.
    ///
    pub async fn transient(&self) -> Shared {
        self.transient.storage.latest().await
    }

    /// Returns a writeable reference to transient storage,
    ///
    pub async fn transient_mut(&self) -> tokio::sync::RwLockWriteGuard<Shared> {
        self.transient.storage.write().await
    }

    /// Spawn a task w/ this context,
    ///
    /// Returns a join-handle if the task was created.
    ///
    pub fn spawn<F>(self, task: impl FnOnce(Context) -> F + 'static) -> SpawnResult
    where
        F: Future<Output = anyhow::Result<Context>> + Send + 'static,
    {
        self.node
            .runtime
            .clone()
            .as_ref()
            .map(|h| h.clone().spawn(task(self)))
    }

    /// Convenience for `PluginOutput::Skip`
    ///
    pub fn skip(&self) -> CallOutput {
        CallOutput::Skip
    }

    /// Convenience for `PluginOutput::Abort(..)`
    ///
    pub fn abort(&self, error: impl Into<anyhow::Error>) -> CallOutput {
        CallOutput::Abort(Err(error.into()))
    }

    /// Retrieves the initialized state of the plugin,
    ///
    /// **Note**: This is the state that was evaluated at the start of the application, when the runmd was parsed.
    ///
    pub async fn initialized<P: BlockObject<Shared> + Default + Clone + Sync + Send + 'static>(
        &self,
    ) -> P {
        self.node()
            .await
            .current_resource::<P>(self.attribute.map(|a| a.transmute()))
            .unwrap_or_default()
    }

    /// Returns any extensions that may exist for this,
    ///
    pub async fn extension<
        C: Send + Sync + 'static,
        P: BlockObject<Shared> + Default + Clone + Sync + Send + 'static,
    >(
        &self,
    ) -> Option<crate::Extension<C, P>> {
        self.node()
            .await
            .current_resource::<crate::Extension<C, P>>(self.attribute.map(|a| a.transmute()))
    }

    /// Schedules garbage collection of the variant,
    ///
    pub(crate) fn garbage_collect(&self) {
        if let (Some(key), Some(_), Ok(storage)) = (
            self.attribute,
            self.variant_id,
            self.node.storage.try_read(),
        ) {
            storage.lazy_dispatch_mut(move |s| {
                debug!(key = key.key(), "Garbage collection");
                s.remove_resource_at(key);
            });
        }
    }
}
