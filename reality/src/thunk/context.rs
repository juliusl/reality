use std::collections::BTreeMap;

use async_stream::stream;
use futures_util::stream::BoxStream;
use futures_util::Future;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::trace;
use uuid::Uuid;

use crate::prelude::Latest;
use crate::AsyncStorageTarget;
use crate::Attribute;
use crate::BlockObject;
use crate::Dispatcher;
use crate::Frame;
use crate::FrameUpdates;
use crate::Node;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;
use crate::Tag;

use super::prelude::*;

/// Struct containing shared context between plugins,
///
#[derive(Clone)]
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
    /// If set, will filter attributes based on thier identifier,
    ///
    pub filter: Option<String>,
    /// If set, will allow a thunk to audit it's usage,
    ///
    pub audit: Option<ThunkAudit>,
    /// Cache storage,
    ///
    /// **Note** Cloning the cache will creates a new branch.
    ///
    __cached: Shared,
}

/// Struct containing audited config information on the thunk,
///
#[derive(Clone)]
pub struct ThunkAudit {
    /// True if the thunk writes to node storage,
    ///
    /// **Consideration** This means that a write lock on shared storage will be taken during thunk execution.
    ///
    pub writes_to_node: bool,
    /// True if the thunk writes to transient storage,
    ///
    pub writes_to_transient: bool,
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
            filter: None,
            audit: None,
            __cached: Shared::default(),
        }
    }
}

impl Context {
    /// Creates a branched thunk context,
    ///
    pub fn branch(&self) -> (Uuid, Self) {
        eprintln!("branching");
        let mut next = self.clone();
        // Create a variant for the type created here
        let variant_id = uuid::Uuid::new_v4();
        if let Some(attr) = next.attribute.as_mut() {
            *attr = attr.branch(variant_id);
        }
        next.variant_id = Some(variant_id);
        (variant_id, next)
    }

    /// Returns the tag value if one was set for this context,
    ///
    pub async fn tag(&self) -> Option<Tag> {
        self.node()
            .await
            .current_resource(self.attribute.map(|a| a.transmute()))
    }

    /// Creates a context w/ a filter set,
    ///
    pub fn filter(&self, filter: impl Into<String>) -> Self {
        let mut with_filter = self.clone();
        with_filter.filter = Some(filter.into());
        with_filter
    }

    /// Resets the filter,
    ///
    pub fn reset_filter(&mut self) {
        self.filter.take();
    }

    /// Reset the transient storage,
    ///
    pub fn reset(&mut self) {
        let handle = self.node.runtime.clone().expect("should have a runtime");
        self.transient = Shared::default().into_thread_safe_with(handle);
    }

    /// Sets the attribute for this context,
    ///
    pub fn set_attribute(&mut self, attribute: ResourceKey<Attribute>) {
        self.attribute = Some(attribute);
    }

    /// Get read access to source storage,
    ///
    pub async fn node(&self) -> tokio::sync::RwLockReadGuard<Shared> {
        self.node.storage.read().await
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
    /// **Note** Will start immediately on the tokio-runtime.
    ///
    pub fn spawn<F>(&self, task: impl FnOnce(Context) -> F + 'static) -> SpawnResult
    where
        F: Future<Output = anyhow::Result<Context>> + Send + 'static,
    {
        self.node
            .runtime
            .clone()
            .as_ref()
            .map(|h| h.clone().spawn(task(self.clone())))
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
    pub async fn initialized<P: Plugin + Sync + Send + 'static>(&mut self) -> P {
        let node = self.node().await;
        let mut output = node
            .current_resource::<P>(self.attribute.map(|a| a.transmute()))
            .unwrap_or_default();
        drop(node);

        // TODO: This might need a bit of thinking
        if let Some(packets) = self
            .node()
            .await
            .resource::<FrameUpdates>(self.attribute.map(|a| a.transmute()))
        {
            trace!("Frame updates enabled, applying field packets");
            for field in packets.0.clone().drain(..).map(|f| f.into_field_owned()) {
                if !output.set_field(field) {
                    error!("Could not set field");
                }
            }
        }

        output
    }

    /// Retrieves the initialized frame state of the plugin,
    ///
    pub async fn initialized_frame(&self) -> Frame {
        self.node()
            .await
            .current_resource::<Frame>(self.attribute.map(|a| a.transmute()))
            .unwrap_or_default()
    }

    /// Initializes and returns a dispatcher for resource T,
    ///
    pub async fn initialized_dispatcher<T: Default + Sync + Send + 'static>(
        &self,
    ) -> Dispatcher<Shared, T> {
        self.node
            .intialize_dispatcher(self.attribute.map(|a| a.transmute()))
            .await
    }

    /// If cached, returns a cached value of P,
    ///
    pub fn cached<P: ToOwned<Owned = P> + Sync + Send + 'static>(&self) -> Option<P> {
        self.__cached
            .current_resource::<P>(self.attribute.map(|a| a.transmute()))
    }

    /// If cached, returns a referenced to the cached value,
    ///
    pub fn cached_ref<P: Sync + Send + 'static>(
        &self,
    ) -> Option<<Shared as StorageTarget>::BorrowResource<'_, P>> {
        self.__cached
            .resource::<P>(self.attribute.map(|a| a.transmute()))
    }

    /// Returns a mutable reference to a cached resource,
    ///
    pub fn cached_mut<P: Sync + Send + 'static>(
        &mut self,
    ) -> Option<<Shared as StorageTarget>::BorrowMutResource<'_, P>> {
        self.__cached
            .resource_mut::<P>(self.attribute.map(|a| a.transmute()))
    }

    /// Returns true if the kv store contains value P at key,
    ///
    pub fn kv_contains<P>(&mut self, key: impl std::hash::Hash) -> bool
    where
        P: Send + Sync + 'static,
    {
        let key = self.attribute.map(|s| s.transmute().branch(key));
        self.__cached.resource::<P>(key).is_some()
    }

    /// Store a resource by key in cache,
    ///
    pub fn store_kv<P>(&mut self, key: impl std::hash::Hash, value: P)
    where
        P: Send + Sync + 'static,
    {
        self.__cached
            .put_resource::<P>(value, self.attribute.map(|s| s.transmute().branch(key)));
    }

    /// Fetch a kv pair by key,
    ///
    pub fn fetch_kv<P>(
        &mut self,
        key: impl std::hash::Hash,
    ) -> Option<(
        ResourceKey<P>,
        <Shared as StorageTarget>::BorrowResource<'_, P>,
    )>
    where
        P: Send + Sync + 'static,
    {
        let key = self.attribute.map(|s| s.transmute().branch(key));
        self.__cached
            .resource::<P>(key)
            .map(|c| (key.expect("should be some"), c))
    }

    /// Fetch a mutable reference to a kv pair by key,
    ///
    pub fn fetch_mut_kv<P>(
        &mut self,
        key: impl std::hash::Hash,
    ) -> Option<(
        ResourceKey<P>,
        <Shared as StorageTarget>::BorrowMutResource<'_, P>,
    )>
    where
        P: Send + Sync + 'static,
    {
        let key = self.attribute.map(|s| s.transmute().branch(key));
        self.__cached
            .resource_mut::<P>(key)
            .map(|c| (key.expect("should be some"), c))
    }

    /// Writes a resource to the cache,
    ///
    pub fn write_cache<R: Sync + Send + 'static>(&mut self, resource: R) {
        self.__cached
            .put_resource(resource, self.attribute.map(|a| a.transmute()))
    }

    /// Takes a cached resource,
    ///
    pub fn take_cache<R: Sync + Send + 'static>(&mut self) -> Option<Box<R>> {
        self.__cached
            .take_resource(self.attribute.map(|a| a.transmute()))
    }

    /// Find and cache a resource,
    ///
    /// - Searches the current context for a resource P
    /// - If include_root is true, searches the root resource key for resource P as well
    ///
    pub async fn find_and_cache<P: ToOwned<Owned = P> + Send + Sync + 'static>(
        &mut self,
        include_root: bool,
    ) {
        let node = self.node().await;
        if let Some(resource) = {
            node.current_resource::<P>(self.attribute.map(|a| a.transmute()))
                .or_else(|| {
                    if include_root {
                        node.current_resource::<P>(None)
                    } else {
                        None
                    }
                })
        } {
            drop(node);
            self.write_cache(resource);
        }
    }

    /// Caches P,
    ///
    pub async fn cache<P: Plugin + Sync + Send + 'static>(&mut self) {
        let next = self.initialized().await;

        self.__cached
            .put_resource::<P>(next, self.attribute.map(|a| a.transmute()))
    }

    /// Scans if a resource exists for the current context,
    ///
    pub async fn scan_node_for<P: ToOwned<Owned = P> + Sync + Send + 'static>(&self) -> Option<P> {
        self.node()
            .await
            .current_resource::<P>(self.attribute.map(|a| a.transmute()))
    }

    /// Scans the entire node for resources of type P,
    ///
    pub async fn scan_node<P: ToOwned<Owned = P> + Sync + Send + 'static>(&self) -> Vec<P> {
        self.node()
            .await
            .stream_attributes()
            .fold(vec![], |mut acc, attr| async move {
                let mut clone = self.clone();
                clone.attribute = Some(attr);

                if let Some(init) = clone.scan_node_for::<P>().await {
                    acc.push(init);
                }
                acc
            })
            .await
    }

    /// Scan and take resourrces of type P from node storage,
    ///
    pub async fn scan_take_node<P: Sync + Send + 'static>(&self) -> Vec<P> {
        let attrs = self
            .node()
            .await
            .stream_attributes()
            .collect::<Vec<_>>()
            .await;
        let mut acc = vec![];
        for attr in attrs {
            let mut clone = self.clone();
            clone.attribute = Some(attr);

            trace!("Scanning to take {:?}", &clone.attribute);
            if let Some(init) = clone.scan_take_node_for::<P>().await {
                acc.push(init);
            }
            trace!("Finished scanning to take {:?}", acc.len());
        }

        acc
    }

    /// Scans if a resource exists for the current context,
    ///
    pub async fn scan_take_node_for<P: Sync + Send + 'static>(&self) -> Option<P> {
        unsafe {
            self.node_mut()
                .await
                .take_resource::<P>(self.attribute.map(|a| a.transmute()))
                .map(|p| *p)
        }
    }

    /// Scans the entire node for resources of type P,
    ///
    pub fn iter_node<P: ToOwned<Owned = P> + Sync + Send + 'static>(&self) -> BoxStream<'_, P> {
        let node = self.node.storage.clone();

        stream! {
            let node = node.clone();
            let attrs = Node(self.node.storage.clone())
                .stream_attributes()
                .fold(vec![], |mut acc, m| async move { acc.push(m); acc }).await;

            for attr in attrs {
                if let Some(init) = node.read().await.current_resource::<P>(Some(attr.transmute())) {
                    yield init;
                }
            }
        }.boxed()
    }

    /// Scans the host for resources of type P,
    ///
    pub async fn scan_host_for<P: Clone + Sync + Send + 'static>(
        &self,
        name: impl AsRef<str>,
    ) -> Option<P> {
        self.host(name.as_ref())
            .await
            .and_then(|h| h.current_resource::<P>(None))
    }

    /// Returns any extensions that may exist for this,
    ///
    pub async fn extension<
        C: Clone + Send + Sync + 'static,
        P: BlockObject<Shared> + Default + Clone + Sync + Send + 'static,
    >(
        &self,
    ) -> Option<crate::Transform<C, P>> {
        self.node()
            .await
            .current_resource::<crate::Transform<C, P>>(self.attribute.map(|a| a.transmute()))
    }

    /// Apply all thunks in attribute order,
    ///
    pub async fn apply_thunks(self) -> anyhow::Result<Self> {
        let node = crate::Node(self.node.storage.clone());
        node.stream_attributes()
            .map(Ok)
            .try_fold(self, Self::apply)
            .await
    }

    /// Apply thunks w/ middleware,
    ///
    pub async fn apply_thunks_with<Fut>(
        self,
        middle: impl Fn(Self, ResourceKey<Attribute>) -> Fut + Copy + Clone + Send + Sync + 'static,
    ) -> anyhow::Result<Self>
    where
        Fut: Future<Output = anyhow::Result<Self>>,
    {
        let node = crate::Node(self.node.storage.clone());
        node.stream_attributes()
            .map(Ok)
            .try_fold(self, |mut acc, next| async move {
                acc.set_attribute(next);

                Self::apply((middle)(acc, next).await?, next).await
            })
            .await
    }

    /// Applies thunk associated to attr,
    ///
    pub async fn apply(mut self, attr: ResourceKey<Attribute>) -> anyhow::Result<Self> {
        // TODO: Might be a hot spot
        {
            debug!("Applying changes to transient storage");
            self.transient_mut().await.drain_dispatch_queues();
            unsafe {
                debug!("Applying changes to node storage");
                self.node_mut().await.drain_dispatch_queues();
            }

            for (host_name, host) in self.hosts.iter_mut() {
                debug!(host_name, "Applying changes to host storage");
                host.storage.write().await.drain_dispatch_queues();
            }
        }

        self.set_attribute(attr);
        let previous = self.clone();

        match self.call().await {
            Ok(Some(tc)) => Ok(tc),
            Ok(None) => Ok(previous),
            Err(err) => Err(err),
        }
    }

    /// Resolves an attribute by path, returns a context if an attribute was found,
    ///
    pub async fn navigate(&self, path: impl AsRef<str>) -> Option<Self> {
        if let Some(located) = self.node.resolve::<Attribute>(path.as_ref()).await {
            let mut navigation = self.clone();
            navigation.set_attribute(located);
            Some(navigation)
        } else {
            None
        }
    }

    /// Schedules garbage collection of the variant,
    ///
    pub fn garbage_collect(&self) {
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

    /// Calls the enable frame thunk fn related to this context,
    ///
    pub async fn enable_frame(&self) -> anyhow::Result<Option<Context>> {
        let thunk = self
            .node()
            .await
            .current_resource::<EnableFrame>(self.attribute.map(|a| a.transmute()));
        if let Some(EnableFrame(thunk)) = thunk {
            (thunk)(self.clone()).await
        } else {
            Err(anyhow::anyhow!("Did not execute thunk"))
        }
    }
}
