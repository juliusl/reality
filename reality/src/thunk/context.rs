use std::sync::Arc;

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

use crate::FrameListener;
use crate::PacketRouter;
use crate::prelude::Latest;
use crate::AsyncStorageTarget;
use crate::Attribute;
use crate::Decoration;
use crate::Dispatcher;
use crate::Frame;
use crate::FrameUpdates;
use crate::HostedResource;
use crate::Node;
use crate::ParsedAttributes;
use crate::ParsedBlock;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;
use crate::StorageTargetKey;
use crate::Tag;

use super::prelude::*;

/// Struct containing shared context between plugins,
///
#[derive(Clone)]
pub struct Context {
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
    pub attribute: StorageTargetKey<Attribute>,
    /// If the context has been branched, this will be the Uuid assigned to the variant,
    ///
    pub variant_id: Option<Uuid>,
    /// Decorations parsed for this context,
    ///
    pub decoration: Option<Decoration>,
    /// Cache storage,
    ///
    /// **Note** Cloning the cache will creates a new branch.
    ///
    pub(crate) __cached: Shared,
}

impl From<AsyncStorageTarget<Shared>> for Context {
    fn from(value: AsyncStorageTarget<Shared>) -> Self {
        let handle = value.runtime.clone().expect("should have a runtime");

        if let Ok(mut storage) = value.storage.try_write() {
            let attribute = storage
                .take_resource(ResourceKey::root())
                .map(|a| *a)
                .unwrap_or(ResourceKey::root());
            Self {
                node: value.clone(),
                attribute,
                transient: Shared::default().into_thread_safe_with(handle),
                cancellation: storage
                    .take_resource(attribute.transmute())
                    .map(|a| *a)
                    .unwrap_or_default(),
                variant_id: storage
                    .take_resource(attribute.transmute())
                    .map(|a| *a)
                    .unwrap_or_default(),
                decoration: storage
                    .take_resource::<Decoration>(attribute.transmute())
                    .map(|a| *a),
                __cached: storage
                    .take_resource(attribute.transmute())
                    .map(|a| *a)
                    .unwrap_or_default(),
            }
        } else {
            Self {
                node: value.clone(),
                attribute: ResourceKey::root(),
                transient: Shared::default().into_thread_safe_with(handle),
                cancellation: CancellationToken::new(),
                variant_id: None,
                decoration: None,
                __cached: Shared::default(),
            }
        }
    }
}

impl AsRef<ThunkContext> for ThunkContext {
    fn as_ref(&self) -> &ThunkContext {
        self
    }
}

impl AsMut<ThunkContext> for ThunkContext {
    fn as_mut(&mut self) -> &mut ThunkContext {
        self
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    /// Returns a new blank thunk context,
    ///
    pub fn new() -> Self {
        Self::from(Shared::default().into_thread_safe())
    }

    /// Unpacks some resource from cached storage,
    ///
    pub fn unpack<T>(&mut self) -> Option<T>
    where
        T: Pack + Sync + Send + Clone + 'static,
    {
        self.cached::<T>().map(|c| c.unpack(&mut self.__cached))
    }

    /// Returns the value of a property decoration if found,
    ///
    pub fn property(&self, name: impl AsRef<str>) -> Option<&String> {
        self.decoration
            .as_ref()
            .and_then(|d| d.comment_properties.as_ref())
            .and_then(|c| c.get(name.as_ref()))
    }

    /// Returns the parsed block,
    ///
    pub async fn parsed_block(&self) -> Option<ParsedBlock> {
        self.node()
            .await
            .current_resource::<ParsedBlock>(StorageTargetKey::root())
    }

    /// Creates a branched thunk context,
    ///
    pub fn branch(&self) -> (Uuid, Self) {
        eprintln!("branching");
        let mut next = self.clone();
        // Create a variant for the type created here
        let variant_id = uuid::Uuid::new_v4();

        let _next = next.attribute.branch(variant_id);
        next.attribute = _next;

        next.variant_id = Some(variant_id);
        (variant_id, next)
    }

    /// Returns the tag value if one was set for this context,
    ///
    pub async fn tag(&self) -> Option<Tag> {
        self.node()
            .await
            .current_resource(self.attribute.transmute())
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
        self.attribute = attribute;
    }

    /// Get read access to source storage,
    ///
    pub async fn node(&self) -> tokio::sync::RwLockReadGuard<Shared> {
        self.node.storage.read().await
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

    /// Returns a readable reference to transient storage,
    ///
    pub async fn transient_ref(&self) -> tokio::sync::RwLockReadGuard<Shared> {
        self.transient.storage.read().await
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

    /// Convenience for `PluginOutput::Skip`
    ///
    pub fn update(self) -> CallOutput {
        CallOutput::Update(Some(self))
    }

    /// Retrieves the initialized state of the plugin,
    ///
    /// **Note**: This is the state that was evaluated at the start of the application, when the runmd was parsed.
    ///
    pub async fn initialized<P: Plugin + Sync + Send + 'static>(&self) -> P {
        let node = self.node()
            .await;

        let plugin = node.current_resource::<P>(self.attribute.transmute())
            .unwrap_or_default();

        drop(node);

        plugin
    }

    /// Returns the packet router initialized for P,
    /// 
    pub async fn router<P: Plugin + Sync + Send + 'static>(&self) -> Option<Arc<PacketRouter<P>>> {
        self.node()
            .await
            .current_resource(self.attribute.transmute())
    }

    /// Returns the current **default** frame listener for plugin P,
    /// 
    /// **Note**: The default frame listener only has a buffer_len of 1.
    /// 
    pub async fn listener<P: Plugin + Sync + Send + 'static>(&self) -> Option<FrameListener<P>> 
    where 
        P::Virtual: NewFn<Inner = P>, 
    {
        self.node()
            .await
            .current_resource(self.attribute.transmute())
    }

    /// Returns the current wire server if initialized,
    /// 
    pub async fn wire_server<P: Plugin + Sync + Send + 'static>(&self) -> Option<Arc<WireServer<P>>> 
    where 
        P::Virtual: NewFn<Inner = P>, 
    {
        self.node()
            .await
            .current_resource(self.attribute.transmute())
    }

    /// Returns the current wire client if initialized,
    /// 
    pub async fn wire_client<P: Plugin + Sync + Send + 'static>(&self) -> Option<WireClient<P>> 
    where 
        P::Virtual: NewFn<Inner = P>, 
    {
        self.node()
            .await
            .current_resource(self.attribute.transmute())
    }

    /// Listens for one packet,
    /// 
    pub async fn listen_one<P: Plugin + Sync + Send + 'static>(self) -> ThunkContext {
        if let Some(router) = self.router().await {
            P::listen_one(router).await;
        }
        self
    }

    /// Creates a new initializer,
    ///
    pub async fn initialize<'a: 'b, 'b, P: Plugin + Sync + Send + 'static>(
        &'a mut self,
    ) -> Initializer<'b, P> {
        let mut init = self.initialized::<P>().await;
        init.sync(self);

        let init = Initializer {
            initialized: init,
            context: self,
        };

        init
    }

    /// Retrieves the initialized frame state of the plugin,
    ///
    pub async fn initialized_frame(&self) -> Frame {
        self.node()
            .await
            .current_resource::<Frame>(self.attribute.transmute())
            .unwrap_or_default()
    }

    /// Returns a dispatcher for resource T,
    ///
    /// **Note** Initializes a new dispatcher if one is not already present,
    ///
    pub async fn dispatcher<T: Default + Sync + Send + 'static>(&self) -> Dispatcher<Shared, T> {
        self.node
            .maybe_intialize_dispatcher(self.attribute.transmute())
            .await
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
            node.current_resource::<P>(self.attribute.transmute())
                .or_else(|| {
                    if include_root {
                        node.current_resource::<P>(ResourceKey::root())
                    } else {
                        None
                    }
                })
        } {
            drop(node);
            self.write_cache(resource);
        }
    }

    /// Returns a clone of the current cache,
    ///
    pub fn clone_cache(&self) -> Shared {
        self.__cached.clone()
    }

    /// Caches the plugin P,
    ///
    pub async fn cache<P: Plugin + Sync + Send + 'static>(&mut self) {
        let next = self.initialized().await;

        self.__cached
            .put_resource::<P>(next, self.attribute.transmute())
    }

    /// Scans if a resource exists for the current context,
    ///
    pub async fn scan_node_for<P: ToOwned<Owned = P> + Sync + Send + 'static>(&self) -> Option<P> {
        self.node()
            .await
            .current_resource::<P>(self.attribute.transmute())
    }

    /// Scans the entire node for resources of type P,
    ///
    pub async fn scan_node<P: ToOwned<Owned = P> + Sync + Send + 'static>(&self) -> Vec<P> {
        self.node()
            .await
            .stream_attributes()
            .fold(vec![], |mut acc, attr| async move {
                let mut clone = self.clone();
                clone.attribute = attr;

                if let Some(init) = clone.scan_node_for::<P>().await {
                    acc.push(init);
                }
                acc
            })
            .await
    }

    /// Finds and returns a thunk context w/ a resource P stored in the node storage,
    ///
    /// **Note** Returns the last plugin found.
    ///
    pub async fn find_node_context<P: ToOwned<Owned = P> + Sync + Send + 'static>(
        &self,
    ) -> Option<ThunkContext> {
        let mut attrs = self
            .node()
            .await
            .stream_attributes()
            .fold(vec![], |mut acc, attr| async move {
                let mut clone = self.clone();
                clone.attribute = attr;

                if let Some(_) = clone.scan_node_for::<P>().await {
                    acc.push(attr);
                }
                acc
            })
            .await;

        if let Some(found) = attrs.pop() {
            let mut tc = self.clone();
            tc.attribute = found;
            Some(tc)
        } else {
            None
        }
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
            clone.attribute = attr;

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
                .take_resource::<P>(self.attribute.transmute())
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
                if let Some(init) = node.read().await.current_resource::<P>(attr.transmute()) {
                    yield init;
                }
            }
        }
        .boxed()
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
    pub async fn navigate(&self, path: impl AsRef<str>) -> Option<HostedResource> {
        let node = self.node().await;
        if let Some(block) = node.resource::<ParsedBlock>(ResourceKey::root()) {
            eprintln!("Looking for resource at: {}", path.as_ref());
            if let Some(hosted_resource) = block.find_resource(path.as_ref().to_string()) {
                eprintln!("Found hosted resource: {:?}", hosted_resource);
                return Some(hosted_resource.clone());
            } else {
                eprintln!("Did not find resource at: {}\n{:#?}", path.as_ref(), block);
            }
        }

        None
    }

    /// Schedules garbage collection of the variant,
    ///
    pub fn garbage_collect(&self) {
        if self.attribute.is_root() {
            return;
        }

        if let (key, Some(_), Ok(storage)) = (
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
            .current_resource::<ThunkFn>(self.attribute.transmute());
        if let Some(thunk) = thunk {
            (thunk)(self.clone()).await
        } else {
            let contains = self
                .node()
                .await
                .contains::<ThunkFn>(self.attribute.transmute());
            Err(anyhow::anyhow!(
                "Did not execute thunk {:?} {contains}",
                self.attribute
            ))
        }
    }

    /// Calls the enable frame thunk fn related to this context,
    ///
    pub async fn enable_frame(&self) -> anyhow::Result<Option<Context>> {
        let thunk = self
            .node()
            .await
            .current_resource::<EnableFrame>(self.attribute.transmute());
        if let Some(EnableFrame(thunk)) = thunk {
            (thunk)(self.clone()).await
        } else {
            Err(anyhow::anyhow!("Did not execute thunk"))
        }
    }

    /// Calls the enable virtual thunk fn related to this context,
    ///
    pub async fn enable_virtual(&self) -> anyhow::Result<Option<Context>> {
        let thunk = self
            .node()
            .await
            .current_resource::<EnableVirtual>(self.attribute.transmute());
        if let Some(EnableVirtual(thunk)) = thunk {
            (thunk)(self.clone()).await
        } else {
            Err(anyhow::anyhow!("Did not execute thunk"))
        }
    }

    /// Prints out debug information on this thunk context,
    ///
    pub fn print_debug_info(&self) {
        if let Some(doc_headers) = self
            .decoration
            .as_ref()
            .and_then(|d| d.doc_headers.as_ref())
        {
            for d in doc_headers {
                eprintln!("{d}");
            }
            eprintln!();
        }
        eprintln!("attribute:      {:?}", self.attribute);
        eprintln!("variant  :      {:?}", self.variant_id);
        eprintln!("--- Decorations ---");
        if let Some(properties) = self
            .decoration
            .as_ref()
            .and_then(|d| d.comment_properties.as_ref())
        {
            for (n, v) in properties {
                eprintln!("{n}: {v}");
            }
        } else {
            eprintln!("None")
        }
        eprintln!("--- Cache State ---");
        eprintln!("# of keys :      {}", self.__cached.len());

        if let Some(frame) = self.cached_ref::<Frame>() {
            eprintln!("--- Frame State ---");
            eprintln!("# of fields:     {}", frame.fields.len());
            // TODO
        }
    }
}

/// A Remote Plugin can depend on initialization of it's state from
/// remote and local dependencies.
///
pub struct Remote;

impl Remote {
    /// Creates plugin P w/ remote features enabled,
    ///
    pub async fn create<P>(self, tc: &mut ThunkContext) -> P
    where
        P: Plugin + Sync + Send + 'static,
    {
        let mut p = tc
            .initialize::<P>()
            .await
            .apply_frame_updates()
            .await
            .apply_decorations()
            .await
            .finish();

        p.sync(&tc);
        if let Some(deco) = tc
            .fetch_kv::<Decoration>(tc.attribute)
            .map(|(_, deco)| deco.clone())
        {
            tc.decoration = Some(deco);
        }
        p
    }
}

/// A Local plugin can depend on local resources for it's initialization,
///
pub struct Local;

impl Local {
    /// Creates plugin local Plugin P,
    ///
    pub async fn create<P>(self, tc: &mut ThunkContext) -> P
    where
        P: Plugin + Sync + Send + 'static,
    {
        let mut plugin = tc
            .initialize::<P>()
            .await
            .apply_decorations()
            .await
            .finish();

        plugin.sync(&tc);
        tc.decoration = tc
            .fetch_kv::<Decoration>(tc.attribute)
            .map(|(_, deco)| deco.clone());
        plugin
    }
}

/// Struct for initializing a plugin,
///
pub struct Initializer<'a, P>
where
    P: Plugin + Sync + Send + 'static,
{
    initialized: P,

    context: &'a mut ThunkContext,
}

impl<'a, P> Initializer<'a, P>
where
    P: Plugin + Sync + Send + 'static,
{
    /// Applies frame updates,
    ///
    pub async fn apply_frame_updates(mut self) -> Initializer<'a, P> {
        debug!("trying to dispatch frame updates");
        let mut dispatcher = self.context.dispatcher::<FrameUpdates>().await;
        
        dispatcher.dispatch_all().await;

        drop(dispatcher);

        debug!("dispatched frame updates");
        // // Drain the frame updates dispatcher
        let node = self.context.node().await;

        debug!("Looking for updates {:?}", self.context.attribute);
        if let Some(packets) = node.resource::<FrameUpdates>(self.context.attribute.transmute()) {
            debug!(
                "Frame updates enabled, applying field packets, {}",
                packets.frame.fields.len()
            );
            for field in packets
                .frame
                .fields
                .iter()
                .map(|f| f.clone().into_field_owned())
            {
                debug!("Applying frame update {:?}", field);
                if !self.initialized.set_field(field) {
                    error!("Could not set field");
                }
            }
        }

        drop(node);

        self
    }

    /// Apply decorations,
    ///
    pub async fn apply_decorations(self) -> Initializer<'a, P> {
        // Index decorations into the current cache,
        {
            let node = self.context.node().await;
            if let Some(parsed) = node.current_resource::<ParsedAttributes>(ResourceKey::root()) {
                drop(node);
                parsed
                    .index_decorations(self.context.attribute, self.context)
                    .await;
            }
        }

        // Set decorations on types that support decorations
        self
    }

    /// Finishes initializing and returns the initialized plugin,
    ///
    pub fn finish(self) -> P {
        self.initialized
    }
}
