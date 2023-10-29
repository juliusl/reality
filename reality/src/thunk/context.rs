use futures_util::Future;
use tokio_util::sync::CancellationToken;

use crate::Shared;
use crate::StorageTarget;
use crate::BlockObject;
use crate::Attribute;
use crate::ResourceKey;
use crate::AsyncStorageTarget;

use super::prelude::*;

/// Struct containing shared context between plugins,
///
pub struct Context {
    /// Source storage mapping to this context,
    ///
    pub source: AsyncStorageTarget<Shared>,
    /// Transient storage target,
    ///
    pub transient: AsyncStorageTarget<Shared>,
    /// Cancellation token that can be used by the engine to signal shutdown,
    ///
    pub cancellation: tokio_util::sync::CancellationToken,
    /// Attribute for this context,
    ///
    pub attribute: Option<ResourceKey<Attribute>>,
}

impl From<AsyncStorageTarget<Shared>> for Context {
    fn from(value: AsyncStorageTarget<Shared>) -> Self {
        let handle = value.runtime.clone().expect("should have a runtime");
        Self {
            source: value,
            attribute: None,
            transient: Shared::default().into_thread_safe_with(handle),
            cancellation: CancellationToken::new(),
        }
    }
}

impl Clone for Context {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            attribute: self.attribute.clone(),
            transient: self.transient.clone(),
            cancellation: self.cancellation.clone(),
        }
    }
}

impl Context {
    /// Reset the transient storage,
    ///
    pub fn reset(&mut self) {
        let handle = self.source.runtime.clone().expect("should have a runtime");
        self.transient = Shared::default().into_thread_safe_with(handle);
    }

    /// Calls the thunk fn related to this context,
    ///
    pub async fn call(&self) -> anyhow::Result<Option<Context>> {
        let _storage = self.source.storage.read().await;
        let storage = _storage.clone();
        let _thunk = storage.resource::<ThunkFn>(self.attribute.map(|a| a.transmute()));
        let thunk = _thunk.as_deref().cloned();
        if let Some(thunk) = thunk {       
            drop(_storage);
            drop(_thunk);
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
    pub async fn source(&self) -> tokio::sync::RwLockReadGuard<Shared> {
        self.source.storage.read().await
    }

    /// Get mutable access to source storage,
    ///
    /// **Note**: Marked unsafe because will mutate the source storage. Source storage is re-used on each execution.
    ///
    pub async unsafe fn source_mut(&self) -> tokio::sync::RwLockWriteGuard<Shared> {
        self.source.storage.write().await
    }

    /// Tries to get access to source storage,
    ///
    pub fn try_source(&self) -> Option<tokio::sync::RwLockReadGuard<Shared>> {
        self.source.storage.try_read().ok()
    }

    /// (unsafe) Tries to get mutable access to source storage,
    ///
    /// **Note**: Marked unsafe because will mutate the source storage. Source storage is re-used on each execution.
    ///
    pub unsafe fn try_source_mut(&mut self) -> Option<tokio::sync::RwLockWriteGuard<Shared>> {
        self.source.storage.try_write().ok()
    }

    /// Returns the transient storage target,
    ///
    /// **Note**: During an operation run dispatch queues are drained before each thunk execution.
    ///
    pub fn transient(&self) -> AsyncStorageTarget<Shared> {
        self.transient.clone()
    }

    /// Returns a writeable reference to transient storage,
    /// 
    pub async fn write_transport(&self) -> tokio::sync::RwLockWriteGuard<Shared> {
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
        self.source
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
    pub async fn initialized<P: BlockObject<Shared> + CallAsync + Default + Clone + Sync + Send + 'static>(&self) -> P {
        self.source
            .storage
            .read()
            .await
            .resource::<P>(self.attribute.clone().map(|a| a.transmute()))
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    /// Returns any extensions that may exist for this,
    ///
    pub async fn extension<
        C: Send + Sync + 'static,
        P:  BlockObject<Shared> + CallAsync + Default + Clone + Sync + Send + 'static,
    >(
        &self,
    ) -> Option<crate::Extension<C, P>> {
        self.source
            .storage
            .read()
            .await
            .resource::<crate::Extension<C, P>>(self.attribute.clone().map(|a| a.transmute()))
            .map(|r| r.clone())
    }
}