use anyhow::anyhow;
use async_stream::stream;
use futures_util::Stream;
use host::Host;
use reality::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::either::Either;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::trace;

use crate::host;
use crate::operation::Operation;
use crate::prelude::wire_ext::WireBus;
use crate::sequence::Sequence;

#[cfg(feature = "hyper-ext")]
use crate::prelude::secure_client;

#[cfg(feature = "hyper-ext")]
use crate::prelude::local_client;

pub struct EngineBuilder {
    /// Plugins to register w/ the Engine
    ///
    plugins: Vec<reality::BlockPlugin<Shared>>,
    /// Runtime builder,
    ///
    runtime_builder: tokio::runtime::Builder,
}

impl EngineBuilder {
    /// Creates a new engine builder,
    ///
    pub fn new(runtime_builder: tokio::runtime::Builder) -> Self {
        Self {
            plugins: vec![],
            runtime_builder,
        }
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn enable<P: Plugin + Default + Clone + ApplyFrame + ToFrame + Send + Sync + 'static>(
        &mut self,
    ) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();
            parser.with_object_type::<Thunk<TransformPlugin<WireBus, P>>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn enable_transform<
        C: SetupTransform<P> + Send + Sync + 'static,
        P: Plugin + Clone + Default + Send + Sync + 'static,
    >(
        &mut self,
    ) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<TransformPlugin<C, P>>>();
        });
    }

    /// Consumes the builder and returns a new engine,
    ///
    pub fn build(mut self) -> Engine {
        #[cfg(feature = "hyper-ext")]
        self.register_with(|p| {
            if let Some(s) = p.storage() {
                s.lazy_put_resource(secure_client(), None);
                s.lazy_put_resource(local_client(), None);
            }
        });

        crate::ext::utility::Utility::register(&mut self);

        let runtime = self.runtime_builder.build().unwrap();

        Engine::new_with(self.plugins, runtime)
    }
}

impl reality::prelude::RegisterWith for EngineBuilder {
    fn register_with(&mut self, plugin: fn(&mut AttributeParser<Shared>)) {
        self.plugins.push(Arc::new(plugin));
    }
}

/// Struct containing engine config/state,
///
/// # Background
///
/// By definition an engine is a sequence of event. This struct will be built by defining events and sequencing in a seperate file using runmd.
///
/// Events will be configured via a plugin model. Plugins will execute when the event is loaded in the order they are defined.
///
/// Plugins are executed as "Thunks" in a "call-by-name" fashion. Plugins belonging to an event share state linearly, meaning after a plugin executes, it can modify state before the next plugin executes.
///
/// An event may have 1 or more plugins.
///
/// ```md
/// # Example engine definition
///
/// ```runmd <application/lifec.engine> mirror
/// <..start> start
/// <..start> cleanup
/// <..loop>
/// ```
///
/// ```runmd <application/lifec.engine.event> start
/// + .runtime
/// ```
///
/// ```runmd <application/lifec.engine.event> cleanup
/// + .runtime
/// ```
///
/// ```
///
pub struct Engine {
    /// Cancelled when the engine is dropped,
    ///
    pub cancellation: CancellationToken,
    /// Host storage,
    ///
    /// All thunk contexts produced by this engine will share this storage target.
    ///
    hosts: BTreeMap<String, crate::host::Host>,
    /// Plugins to register w/ the Project
    ///
    plugins: Vec<reality::BlockPlugin<Shared>>,
    /// Wrapped w/ a runtime so that it can be dropped properly
    ///
    runtime: Option<tokio::runtime::Runtime>,
    /// Operations mapped w/ this engine,
    ///
    operations: BTreeMap<String, Operation>,
    /// Sequences mapped w/ this engine
    ///
    sequences: BTreeMap<String, Sequence>,
    /// Engine handle that can be used to send packets to this engine,
    ///
    handle: EngineHandle,
    /// Packet receiver,
    ///
    packet_rx: tokio::sync::mpsc::UnboundedReceiver<EnginePacket>,
    /// Workspace,
    ///
    workspace: Option<Workspace>,
}

impl Engine {
    /// Returns the default host for this engine,
    ///
    pub fn default_host(&self) -> Host {
        let mut default_host = Host::default();
        default_host.children = self.hosts.clone();
        default_host
    }

    /// Returns an iterator over hosts,
    ///
    pub fn iter_hosts(&self) -> impl Iterator<Item = (&String, &Host)> {
        self.hosts.iter()
    }

    /// Creates a new engine builder,
    ///
    pub fn builder() -> EngineBuilder {
        let mut runtime = tokio::runtime::Builder::new_multi_thread();
        runtime.enable_all();

        EngineBuilder::new(runtime)
    }

    /// Registers a plugin w/ this engine,
    ///
    pub fn enable<P: Plugin + Default + Clone + ApplyFrame + ToFrame + Send + Sync + 'static>(
        &mut self,
    ) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();

            #[cfg(feature = "wire-ext")]
            parser.with_object_type::<Thunk<TransformPlugin<WireBus, P>>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn enable_transform<
        C: SetupTransform<P> + Send + Sync + 'static,
        P: Plugin + Clone + Default + Send + Sync + 'static,
    >(
        &mut self,
    ) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<TransformPlugin<C, P>>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    #[inline]
    pub fn register_with(&mut self, plugin: fn(&mut AttributeParser<Shared>)) {
        self.plugins.push(Arc::new(plugin));
    }

    /// Creates a new engine,
    ///
    /// **Note** By default creates a new multi_thread runtime w/ all features enabled
    ///
    pub fn new() -> Self {
        let mut runtime = tokio::runtime::Builder::new_multi_thread();
        runtime.enable_all();

        let runtime = runtime.build().expect("should have an engine");
        Engine::new_with(vec![], runtime)
    }

    /// Creates a new engine w/ runtime,
    ///
    pub fn new_with(
        plugins: Vec<reality::BlockPlugin<Shared>>,
        runtime: tokio::runtime::Runtime,
    ) -> Self {
        let (sender, rx) = tokio::sync::mpsc::unbounded_channel();
        let hosts = BTreeMap::new();

        Engine {
            hosts,
            plugins,
            runtime: Some(runtime),
            cancellation: CancellationToken::new(),
            operations: BTreeMap::new(),
            sequences: BTreeMap::new(),
            handle: EngineHandle {
                sender: Arc::new(sender),
                operations: BTreeMap::new(),
                sequences: BTreeMap::new(),
                hosts: BTreeMap::new(),
                cache: Shared::default(),
                __spawned: None,
            },
            packet_rx: rx,
            workspace: None,
        }
    }

    /// Creates a new context on this engine,
    ///
    /// **Note** Each time a thunk context is created a new output storage target is generated, however the original storage target is used.
    ///
    pub async fn new_context(&self, storage: Arc<tokio::sync::RwLock<Shared>>) -> ThunkContext {
        trace!("Created new context");

        let mut context = ThunkContext::from(AsyncStorageTarget::from_parts(
            storage,
            self.runtime
                .as_ref()
                .map(|r| r.handle().clone())
                .expect("should have a runtime"),
        ));
        context.hosts = self.hosts.iter().fold(BTreeMap::new(), |mut acc, h| {
            if let Some(storage) = h.1.host_storage.clone() {
                acc.insert(h.0.to_string(), storage);
            }
            acc
        });
        context.cancellation = self.cancellation.child_token();
        context
    }

    /// Compiles operations from the parsed project,
    ///
    pub async fn compile(mut self, workspace: Workspace) -> Self {
        let storage = Shared::default();
        let mut project = Project::new(storage);
        project.add_block_plugin(None, None, |_| {});

        let plugins = self.plugins.clone();
        project.add_node_plugin("operation", move |name, tag, target| {
            let name = name
                .map(|n| n.to_string())
                .unwrap_or(format!("{}", uuid::Uuid::new_v4()));

            if let Some(mut storage) = target.storage_mut() {
                storage.put_resource(Operation::new(name, tag.map(|t| t.to_string())), None)
            }

            for p in plugins.iter() {
                p(target);
            }
        });

        project.add_node_plugin("sequence", move |name, tag, target| {
            let name = name
                .map(|n| n.to_string())
                .unwrap_or(format!("{}", uuid::Uuid::new_v4()));

            Sequence::parse(target, "");

            if let Some(last) = target.attributes.last().cloned() {
                if let Some(mut storage) = target.storage_mut() {
                    storage.drain_dispatch_queues();
                    if let Some(mut seq) = storage.resource_mut(Some(last.transmute::<Sequence>()))
                    {
                        seq.deref_mut().name = name;
                        seq.deref_mut().tag = tag.map(|t| t.to_string());
                    }
                    storage.put_resource(last.transmute::<Sequence>(), None);
                }
            }
        });

        project.add_node_plugin("host", move |name, tag, target| {
            let name = name
                .map(|n| n.to_string())
                .unwrap_or(format!("{}", uuid::Uuid::new_v4()));

            Host::parse(target, &name);

            if let Some(last) = target.attributes.last().cloned() {
                if let Some(mut storage) = target.storage_mut() {
                    storage.drain_dispatch_queues();
                    if let Some(mut host) = storage.resource_mut(Some(last.transmute::<Host>())) {
                        host._tag = tag.map(|t| t.to_string());
                    }
                    storage.put_resource(last.transmute::<Host>(), None);
                }
            }
        });

        if let Some(project) = workspace
            .compile(project)
            .await
            .ok()
            .and_then(|mut w| w.project.take())
        {
            let handle = self.handle();

            let nodes = project.nodes.into_inner().unwrap();

            // Extract hosts
            for (_, target) in nodes.iter() {
                target.write().await.drain_dispatch_queues();

                let storage = target.latest().await;
                let hostkey = storage.current_resource::<ResourceKey<Host>>(None);
                if let Some(host) = storage.current_resource::<Host>(hostkey) {
                    if let Some(previous) = self.hosts.insert(
                        host.name.to_string(),
                        host.bind(Shared::default().into_thread_safe_with(handle.clone())),
                    ) {
                        info!(address = previous.name, "Replacing host");
                    }
                }

                target.write().await.drain_dispatch_queues();
            }

            // Extract operations
            for (_, target) in nodes.iter() {
                if let Some(mut operation) =
                    target.latest().await.current_resource::<Operation>(None)
                {
                    operation.bind(self.new_context(target.clone()).await);
                    if let Some(previous) = self.operations.insert(operation.address(), operation) {
                        info!(address = previous.address(), "Replacing operation");
                    }
                }

                target.write().await.drain_dispatch_queues();

                let storage = target.latest().await;
                let seqkey = storage.current_resource::<ResourceKey<Sequence>>(None);

                if let Some(sequence) = storage.current_resource::<Sequence>(seqkey) {
                    if let Some(previous) = self.sequences.insert(
                        sequence.address(),
                        sequence.bind(self.new_context(target.clone()).await),
                    ) {
                        info!(address = previous.address(), "Replacing sequence");
                    }
                }

                target.write().await.drain_dispatch_queues();
            }

            for (_, target) in nodes.iter() {
                target
                    .write()
                    .await
                    .put_resource(self.engine_handle(), None);
            }
        }

        println!("Got hosts {:?}", self.hosts);

        self.workspace = Some(workspace);
        self
    }

    /// Runs an operation by address,
    ///
    pub async fn run(&self, address: impl AsRef<str>) -> anyhow::Result<ThunkContext> {
        if let Some(operation) = self.operations.get(address.as_ref()) {
            operation.execute().await
        } else {
            Err(anyhow!("Operation does not exist"))
        }
    }

    /// Returns an iterator over operations,
    ///
    pub fn iter_operations(&self) -> impl Iterator<Item = (&String, &Operation)> {
        self.operations.iter()
    }

    /// Returns an iterator over sequences,
    ///
    pub fn iter_sequences(&self) -> impl Iterator<Item = (&String, &Sequence)> {
        self.sequences.iter()
    }

    /// Returns a tokio runtime handle,
    ///
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime
            .as_ref()
            .map(|r| r.handle().clone())
            .unwrap_or(Handle::current())
    }

    /// Returns an engine handle,
    ///
    pub fn engine_handle(&self) -> EngineHandle {
        let mut h = self.handle.clone();
        h.operations = self.operations.clone();
        h.sequences = self.sequences.clone();
        h.hosts = self.hosts.clone();
        h
    }

    /// Get host compiled by this engine,
    ///
    pub fn get_host(&self, name: impl AsRef<str>) -> Option<host::Host> {
        self.hosts.get(name.as_ref()).cloned().map(|mut h| {
            h.handle = Some(self.engine_handle());
            h
        })
    }

    /// Takes ownership of the engine and starts listening for packets,
    ///
    pub fn spawn(
        self,
        middleware: impl Fn(&mut Engine, EnginePacket) -> Option<EnginePacket> + Send + Sync + 'static,
    ) -> JoinHandle<anyhow::Result<Self>> {
        eprintln!("Starting engine packet listener");
        tokio::spawn(self.handle_packets(middleware))
    }

    /// Starts handling engine packets,
    ///
    pub async fn handle_packets(
        mut self,
        middleware: impl Fn(&mut Engine, EnginePacket) -> Option<EnginePacket>,
    ) -> anyhow::Result<Self> {
        while let Some(packet) = self.packet_rx.recv().await {
            if self.cancellation.is_cancelled() {
                break;
            }

            if let Some(packet) = middleware(&mut self, packet) {
                println!("Handling packet {:?}", packet.action);
                match packet.action {
                    Action::Run { address, tx } => {
                        println!("Running {}", address);
                        info!(address, "Running operation");
                        let action = self
                            .operations
                            .get(&address)
                            .map(|o| Either::Left(o.clone()))
                            .or(self
                                .sequences
                                .get(&address)
                                .map(|s| Either::Right(s.clone())));
                        if let (Some(tx), Some(action)) = (tx, action) {
                            tx.send(action).map_err(|_| anyhow!("Channel is closed"))?;
                        }
                    }
                    Action::Compile { relative, content } => {
                        info!("Compiling content");
                        if let Some(mut workspace) = self.workspace.take() {
                            workspace.add_buffer(relative, content);
                            self = self.compile(workspace).await;
                        }
                    }
                    Action::Sync { mut tx } => {
                        info!("Syncing engine handle");
                        if let Some(tx) = tx.take() {
                            if let Err(_) = tx.send(self.engine_handle()) {
                                error!("Could not send updated handle");
                            }
                        }
                    }
                    Action::Shutdown(delay) => {
                        info!(delay_ms = delay.as_millis(), "Shutdown requested");
                        tokio::time::sleep(delay).await;
                        self.cancellation.cancel();
                        break;
                    }
                }
            }
        }

        Ok(self)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.cancellation.cancel();

        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// Struct containing instructions to execute w/ an engine,
///
#[derive(Debug, Serialize, Deserialize)]
pub struct EnginePacket {
    /// Address of the operation to execute,
    ///
    action: Action,
}

/// Enumeration of actions that can be requested by a packet,
///
#[derive(Serialize, Deserialize)]
pub enum Action {
    /// Runs an operation on the engine,
    ///
    Run {
        /// Address of the action to run,
        ///
        address: String,
        /// Channel to transmit the result back to the sender,
        ///
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<Either<Operation, Sequence>>>,
    },
    /// Gets an updated engine handle,
    ///
    Sync {
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<EngineHandle>>,
    },
    /// Compiles the operations from a project,
    ///
    Compile { relative: String, content: String },
    /// Requests the engine to
    ///
    Shutdown(tokio::time::Duration),
}

impl Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Run { address, tx } => f
                .debug_struct("Run")
                .field("address", address)
                .field("has_tx", &tx.is_some())
                .finish(),
            Self::Compile { relative, content } => f
                .debug_struct("Compile")
                .field("relative", relative)
                .field("content", content)
                .finish(),
            Self::Sync { tx } => f
                .debug_struct("Sync")
                .field("has_tx", &tx.is_some())
                .finish(),
            Self::Shutdown(arg0) => f.debug_tuple("Shutdown").field(arg0).finish(),
        }
    }
}

/// Handle for communicating and sending work packets to an engine,
///
pub struct EngineHandle {
    /// Sends engine packets to the engine,
    ///
    sender: Arc<tokio::sync::mpsc::UnboundedSender<EnginePacket>>,
    /// Map of operations,
    ///
    pub operations: BTreeMap<String, Operation>,
    /// Map of sequences,
    ///
    pub sequences: BTreeMap<String, Sequence>,
    /// Map of hosts,
    ///
    pub hosts: BTreeMap<String, Host>,
    /// Local cache for the handle,
    /// 
    pub cache: Shared,
    /// Actively running task,
    ///
    __spawned: Option<(Instant, JoinHandle<anyhow::Result<Self>>)>,
}

impl Clone for EngineHandle {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            operations: self.operations.clone(),
            sequences: self.sequences.clone(),
            hosts: self.hosts.clone(),
            cache: self.cache.clone(),
            __spawned: None,
        }
    }
}

impl Debug for EngineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineHandle")
            .field("sender", &self.sender)
            .finish()
    }
}

impl EngineHandle {
    /// Runs an operation by sending a packet and waits for a response,
    ///
    pub async fn run(&self, address: impl Into<String>) -> anyhow::Result<ThunkContext> {
        let (tx, rx) =
            tokio::sync::oneshot::channel::<tokio_util::either::Either<Operation, Sequence>>();

        let packet = EnginePacket {
            action: Action::Run {
                address: address.into(),
                tx: Some(tx),
            },
        };

        self.sender.send(packet)?;

        rx.await?.await
    }

    /// Compiles content,
    ///
    pub async fn compile(
        &self,
        relative: impl Into<String>,
        content: impl Into<String>,
    ) -> anyhow::Result<()> {
        let packet = EnginePacket {
            action: Action::Compile {
                relative: relative.into(),
                content: content.into(),
            },
        };

        self.sender.send(packet)?;

        Ok(())
    }

    /// Sends a signal for the engine to shutdown,
    ///
    pub async fn shutdown(&self, delay: tokio::time::Duration) -> anyhow::Result<()> {
        let packet = EnginePacket {
            action: Action::Shutdown(delay),
        };

        self.sender.send(packet)?;
        Ok(())
    }

    /// Synchronize the state of this handle,
    ///
    pub async fn sync(&self) -> anyhow::Result<EngineHandle> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        let packet = EnginePacket {
            action: Action::Sync { tx: Some(tx) },
        };

        self.sender.send(packet)?;

        Ok(rx.await?)
    }

    /// Scans node storage for resource T,
    ///
    pub fn scan_take_nodes<T>(&self) -> impl Stream<Item = T> + '_
    where
        T: Send + Sync + 'static,
    {
        stream! {
            for (_, op) in self.operations.iter() {
                if let Some(tc) = op.context() {
                    for resource in tc.scan_take_node::<T>().await {
                        yield resource;
                    }
                }
            }
        }
    }

    pub fn scan_nodes<T>(&self) -> impl Stream<Item = T> + '_
    where
        T: ToOwned<Owned = T> + Send + Sync + 'static,
    {
        stream! {
            for (_, op) in self.operations.iter() {
                if let Some(tc) = op.context() {
                    for resource in tc.scan_node::<T>().await {
                        yield resource;
                    }
                }
            }
        }
    }

    /// Scans host storage for resource T,
    ///
    pub fn scan_host<T>(&self, host: &'static str) -> impl Stream<Item = T> + '_
    where
        T: ToOwned<Owned = T> + Send + Sync + 'static,
    {
        stream! {
            for (_, op) in self.operations.iter() {
                if let Some(tc) = op.context() {
                    if let Some(host) = tc.host(host).await {
                        if let Some(r) = host.current_resource::<T>(tc.attribute.map(|a| a.transmute())) {
                            yield r;
                        }
                    }
                }
            }
        }
    }

    /// Navigates to an address,
    ///
    pub async fn navigate(
        &self,
        operation: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> anyhow::Result<ThunkContext> {
        if let Some(op) = self.operations.get(operation.as_ref()) {
            return op.navigate(path.as_ref().trim_start_matches('/')).await;
        }

        Err(anyhow::anyhow!(
            "Could not find address: {} {}",
            operation.as_ref(),
            path.as_ref()
        ))
    }

    /// Returns true if the spawn closure was successfully spawned,
    /// 
    pub fn spawn(&mut self, spawn: impl FnOnce(EngineHandle) -> JoinHandle<anyhow::Result<Self>> + 'static) -> Option<Instant> {
        if self.__spawned.is_some() {
            return None;
        }

        let start = Instant::now();
        self.__spawned = Some((start, spawn(self.clone())));
        Some(start)
    }

    /// Returns true if an internal task is running,
    /// 
    pub fn is_running(&self) -> bool {
        self.__spawned.is_some()
    }

    /// Returns Some(true) if the internal task is still running,
    /// 
    pub fn is_finished(&self) -> Option<bool> {
        self.__spawned.as_ref().map(|(_, s)| s.is_finished())
    }

    /// Updates in place,
    /// 
    /// returns an error if there is not currently a running task,
    /// 
    /// or; if the task could not be complete successfully, 
    /// 
    /// or; if the task completed but returned an error.
    /// 
    pub fn wait_for_finish(&mut self, instant: Instant) -> anyhow::Result<EngineHandle> {
        if let Some((started, _)) = self.__spawned.as_ref() {
            if instant != *started {
                return Err(anyhow!(""));
            }
        }

        if let Some((_, spawned)) = self.__spawned.take() {
            futures::executor::block_on(async { spawned.await })?
        } else {
            Err(anyhow!("No running task"))
        }
    }

    /// Cancels any spawned join handles from this engine handle,
    /// 
    pub fn cancel(&mut self) {
        if let Some((_, spawned)) = self.__spawned.take() {
            spawned.abort();
        }
    }
}
