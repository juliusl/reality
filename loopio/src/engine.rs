use anyhow::anyhow;
use host::Host;
use reality::prelude::*;
use reality::RwLock;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;

use crate::background_work::BackgroundWorkEngineHandle;
use crate::host;
use crate::operation;
use crate::operation::Operation;
use crate::prelude::Action;
use crate::prelude::Address;
use crate::sequence::Sequence;

#[cfg(feature = "hyper-ext")]
use crate::prelude::secure_client;

#[cfg(feature = "hyper-ext")]
use crate::prelude::local_client;

pub struct DefaultEngine;

impl DefaultEngine {
    pub fn new(self) -> Engine {
        Engine::builder().build()
    }
}

pub struct EngineBuilder {
    /// Plugins to register w/ the Engine
    ///
    plugins: Vec<reality::BlockPlugin<Shared>>,
    /// Runtime builder,
    ///
    pub(crate) runtime_builder: tokio::runtime::Builder,
    /// Workspace,
    /// 
    pub(crate) workspace: Workspace,
}

impl EngineBuilder {
    /// Creates a new engine builder,
    ///
    pub fn new(runtime_builder: tokio::runtime::Builder) -> Self {
        Self {
            plugins: vec![],
            runtime_builder,
            workspace: EmptyWorkspace.workspace(),
        }
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn enable<P: Plugin + Default + Clone + ToFrame + Send + Sync + 'static>(&mut self) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();
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

    /// Sets a workspace,
    /// 
    pub fn set_workspace(&mut self, workspace: Workspace) {
        self.workspace = workspace;
    }

    /// Gets a mutable reference to the workspace,
    /// 
    pub fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspace
    }

    pub async fn compile(self) -> Engine {
        let workspace = self.workspace.clone();
        let engine = self.build();

        let engine = engine.compile(workspace).await;
        engine
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
    /// Plugins to register w/ the Project
    ///
    plugins: Vec<reality::BlockPlugin<Shared>>,
    /// Host storage,
    ///
    /// All thunk contexts produced by this engine will share this storage target.
    ///
    hosts: BTreeMap<String, crate::host::Host>,
    /// Operations mapped w/ this engine,
    ///
    operations: BTreeMap<String, Operation>,
    /// Sequences mapped w/ this engine
    ///
    sequences: BTreeMap<String, Sequence>,
    /// Current nodes,
    ///
    pub nodes: HashMap<ResourceKey<reality::attributes::Node>, Arc<RwLock<Shared>>>,
    /// Engine handle that can be used to send packets to this engine,
    ///
    handle: EngineHandle,
    /// Packet receiver,
    ///
    packet_rx: tokio::sync::mpsc::UnboundedReceiver<EnginePacket>,
    /// Wrapped w/ a runtime so that it can be dropped properly
    ///
    runtime: Option<tokio::runtime::Runtime>,
    /// Workspace,
    ///
    workspace: Option<Workspace>,
    /// Pasred block,
    ///
    pub block: Option<ParsedBlock>,
    /// Internal hosted resources,
    ///
    __internal_resources: InternalResources,
}

impl Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("hosts", &self.hosts)
            .field("operations", &self.operations)
            .field("sequences", &self.sequences)
            .finish()
    }
}

impl Engine {
    /// Returns an iterator over hosts,
    ///
    #[inline]
    pub fn iter_hosts(&self) -> impl Iterator<Item = (&String, &Host)> {
        self.hosts.iter()
    }

    /// Creates a new engine builder,
    ///
    #[inline]
    pub fn builder() -> EngineBuilder {
        let mut runtime = tokio::runtime::Builder::new_multi_thread();
        runtime.enable_all();

        EngineBuilder::new(runtime)
    }

    /// Registers a plugin w/ this engine,
    ///
    #[inline]
    pub fn enable<P: Plugin + Default + Clone + ToFrame + Send + Sync + 'static>(&mut self) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();
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
    #[inline]
    pub(crate) fn new() -> Self {
        let mut runtime = tokio::runtime::Builder::new_multi_thread();
        runtime.enable_all();

        let runtime = runtime.build().expect("should have an engine");
        Engine::new_with(vec![], runtime)
    }

    /// Creates a new engine w/ runtime,
    ///
    #[inline]
    pub(crate) fn new_with(
        plugins: Vec<reality::BlockPlugin<Shared>>,
        runtime: tokio::runtime::Runtime,
    ) -> Self {
        let (sender, rx) = tokio::sync::mpsc::unbounded_channel();
        let hosts = BTreeMap::new();
        let cancellation = CancellationToken::new();
        Engine {
            hosts,
            plugins,
            runtime: Some(runtime),
            cancellation: cancellation.clone(),
            operations: BTreeMap::new(),
            sequences: BTreeMap::new(),
            handle: EngineHandle {
                sender: Arc::new(sender),
                background_work: None,
            },
            packet_rx: rx,
            block: None,
            workspace: None,
            nodes: HashMap::new(),
            __internal_resources: BTreeMap::new(),
        }
    }

    fn add_node_plugin<T>(
        name: Option<&str>,
        tag: Option<&str>,
        target: &mut AttributeParser<Shared>,
    ) where
        T: Plugin + BlockObject<Shared> + crate::prelude::Action + SetIdentifiers,
    {
        let name = name
            .map(|n| n.to_string())
            .unwrap_or(format!("{}", uuid::Uuid::new_v4()));

        T::parse(target, &name);

        let node = target.attributes.node;
        if let Some(last) = target.attributes.last().cloned() {
            if let Some(mut storage) = target.storage_mut() {
                storage.drain_dispatch_queues();
                let mut address = None;
                if let Some(mut seq) = storage.resource_mut(Some(last.transmute::<T>())) {
                    seq.bind_node(node.transmute());
                    T::set_identifiers(&mut seq, &name, tag.map(|t| t.to_string()).as_ref());
                    address = Some(Address::new(seq.address()));
                }

                if let Some(address) = address {
                    eprintln!("Setting address {:?}", address);
                    storage.put_resource(address, None);
                }

                storage.put_resource::<ThunkFn>(<T as Plugin>::call, Some(last.transmute()));
                storage.put_resource::<EnableFrame>(
                    EnableFrame(<T as Plugin>::enable_frame),
                    Some(last.transmute()),
                );
                storage.put_resource(last.transmute::<T>(), None);
            }
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
            let node = target.attributes.node;
            if let Some(mut storage) = target.storage_mut() {
                let mut operation = Operation::new(name, tag.map(|t| t.to_string()));
                operation.bind_node(node.transmute());

                if let Ok(address) = Address::from_str(&operation.address()) {
                    eprintln!("Adding address for operation -- {}", address);
                    storage.put_resource(address, None);
                } else {
                    eprintln!("Could not add address for {}", (&operation.address()));
                }

                storage.put_resource::<ThunkFn>(<Operation as Plugin>::call, None);
                storage.put_resource::<EnableFrame>(
                    EnableFrame(<Operation as Plugin>::enable_frame),
                    None,
                );
                storage.put_resource(operation, None);
            }

            for p in plugins.iter() {
                p(target);
            }
        });

        project.add_node_plugin("sequence", Self::add_node_plugin::<Sequence>);
        project.add_node_plugin("host", Self::add_node_plugin::<Host>);

        if let Some(project) = workspace
            .compile(project)
            .await
            .ok()
            .and_then(|mut w| w.project.take())
        {
            let nodes = project.nodes.latest().await;

            let mut host_actions = vec![];

            // Extract hosts
            for (_, target) in nodes.iter() {
                target.write().await.drain_dispatch_queues();
                let storage = target.latest().await;
                let hostkey = storage.current_resource::<ResourceKey<Host>>(None);
                if let Some(_) = storage.current_resource::<Host>(hostkey) {
                    // Since new_context set the host map, earlier hosts are available to later hosts
                    let mut context = self
                        .new_context(Arc::new(tokio::sync::RwLock::new(storage)))
                        .await;
                    context.attribute = hostkey.map(|s| s.transmute());

                    let mut host = Interactive.create::<Host>(&mut context).await;
                    host.bind(context);
                    // Find actions defined by the host for adding to the parsed block later
                    for (dec, a) in host
                        .action
                        .iter()
                        .filter(|a| a.value().is_some())
                        .map(|a| (a.decorations().cloned(), a.value.clone().unwrap()))
                    {
                        host_actions.push((host.name.to_string(), a, dec));
                    }

                    if let Some(hostkey) = hostkey {
                        host.bind_plugin(hostkey.transmute());
                    }
                    if let Some(previous) = self.hosts.insert(host.name.to_string(), host) {
                        warn!(address = previous.name, "Replacing host");
                    }
                }
                target.write().await.drain_dispatch_queues();
            }

            // Extract actions
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
                if let Some(_) = storage.current_resource::<Sequence>(seqkey) {
                    let mut context = self.new_context(target.clone()).await;
                    context.attribute = seqkey.map(|a| a.transmute());

                    let mut sequence = Interactive.create::<Sequence>(&mut context).await;
                    sequence.bind(context);
                    if let Some(seqkey) = seqkey {
                        sequence.bind_plugin(seqkey.transmute());
                    }

                    if let Some(previous) = self.sequences.insert(sequence.address(), sequence) {
                        info!(address = previous.address(), "Replacing sequence");
                    }
                }
                target.write().await.drain_dispatch_queues();
            }

            // Add EngineHandle to all nodes,
            for (_, target) in nodes.iter() {
                target
                    .write()
                    .await
                    .put_resource(self.engine_handle(), None);
            }

            // Add ParsedBlock to all nodes,
            if let Ok(mut block) = project.parsed_block().await {
                // Bind all pending address to the block first
                for (node, target) in nodes.iter() {
                    if let Some(address) = target.read().await.resource::<Address>(None) {
                        block.bind_node_to_path(node.transmute(), address.to_string());
                    }
                }

                // Bind all hosted resources to it's own thunk context
                // This creates a seperate thunk context which shares node storage but has it's own cache
                for (host_name, address, deco) in host_actions {
                    trace!(
                        host_name,
                        address = address.to_string(),
                        "Adding hosted resource --\n{:#?}",
                        deco
                    );
                    if let Some(node) = block.paths.get(&address.node_address()) {
                        if let Some(node_storage) = nodes.get(&node.transmute()).cloned() {
                            if let Some(resource) = block
                                .nodes
                                .get(&node.transmute())
                                .and_then(|n| n.paths.get(address.path()))
                                .cloned()
                            {
                                let address = address.clone().with_host(host_name);
                                let hosted_resource = block.bind_resource_path(
                                    address.to_string(),
                                    node.transmute(),
                                    resource,
                                    deco,
                                );

                                let mut context = self.new_context(node_storage.clone()).await;
                                context.set_attribute(resource);
                                hosted_resource.bind(context);
                            }
                        }
                    }
                }

                // Share the block w/ all nodes
                for (_, target) in nodes.iter() {
                    target.write().await.put_resource(block.clone(), None);
                }

                for (addr, host) in self.hosts.iter_mut() {
                    unsafe {
                        let mut node = host.context_mut().node_mut().await;
                        node.put_resource(block.clone(), None);
                    }

                    self.__internal_resources.insert(
                        Address::from_str(addr).unwrap(),
                        host.into_hosted_resource(),
                    );
                }

                for (addr, op) in self.operations.iter_mut() {
                    unsafe {
                        let mut node = op.context_mut().node_mut().await;
                        node.put_resource(block.clone(), None);
                    }
                    self.__internal_resources
                        .insert(Address::from_str(addr).unwrap(), op.into_hosted_resource());
                }

                for (addr, seq) in self.sequences.iter_mut() {
                    unsafe {
                        let mut node = seq.context_mut().node_mut().await;
                        node.put_resource(block.clone(), None);
                    }
                    self.__internal_resources
                        .insert(Address::from_str(addr).unwrap(), seq.into_hosted_resource());
                }

                for (address, resource) in block.resource_paths.iter() {
                    if let Ok(address) = Address::from_str(address) {
                        self.__internal_resources
                            .insert(address.clone(), resource.clone());
                        self.__internal_resources
                            .insert(address.with_host("engine"), resource.clone());
                    }
                }

                if let Some(_block) = self.block.as_mut() {
                    _block.nodes.extend(block.nodes);
                    _block.paths.extend(block.paths);
                    _block.resource_paths.extend(block.resource_paths);
                } else {
                    self.block = Some(block);
                }
            }

            self.nodes = nodes;
        }

        println!("Got hosts {:#?}", self.hosts);

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

    /// Returns the parsed block,
    ///
    pub fn block(&self) -> Option<&ParsedBlock> {
        self.block.as_ref()
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
        self.handle.clone()
    }

    /// Get host compiled by this engine,
    ///
    pub async fn get_host(&self, name: impl AsRef<str>) -> Option<host::Host> {
        if let Some(mut host) = self.hosts.get(name.as_ref()).cloned() {
            // TODO -- This is a bit wonky
            // -- A little less wonky but still kind of weird
            unsafe {
                host.context_mut()
                    .node_mut()
                    .await
                    .put_resource(self.engine_handle(), None);
            }
            Some(host)
        } else {
            None
        }
    }

    pub async fn get_sequence(&self, name: impl AsRef<str>) -> Option<Sequence> {
        if let Some(mut seq) = self.sequences.get(name.as_ref()).cloned() {
            unsafe {
                seq.context_mut()
                    .node_mut()
                    .await
                    .put_resource(self.engine_handle(), None);
            }
            Some(seq)
        } else {
            None
        }
    }

    pub async fn get_operation(&self, name: impl AsRef<str>) -> Option<operation::Operation> {
        if let Some(mut operation) = self.operations.get(name.as_ref()).cloned() {
            unsafe {
                operation
                    .context_mut()
                    .node_mut()
                    .await
                    .put_resource(self.engine_handle(), None);
            }
            Some(operation)
        } else {
            None
        }
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
                trace!("Handling packet {:?}", packet.action);
                match packet.action {
                    EngineAction::Call { address, mut tx } => {
                        info!(address, "Looking up hosted resource");
                        match Address::from_str(&address) {
                            Ok(address) => {
                                // Lookup from the internal resources for a plugin
                                if let Some(resource) = self.__internal_resources.get(&address) {
                                    trace!(address = resource.address(), "Found resource");
                                    if let Some(tx) = tx.take() {
                                        let mut resource = resource.clone();
                                        unsafe {
                                            resource
                                                .context_mut()
                                                .node_mut()
                                                .await
                                                .put_resource(self.engine_handle(), None);
                                        }

                                        resource.context_mut().write_cache(self.engine_handle());

                                        if let Some(node) = self.nodes.get(&resource.node_rk()) {
                                            let mut tc = self.new_context(node.clone()).await;
                                            tc.attribute = resource.plugin_rk();
                                            resource.bind(tc);
                                        }

                                        if let Err(_) = tx.send(resource.spawn().into()) {
                                            // TODO
                                            eprintln!("Could not send spawn result");
                                        }
                                    }
                                } else {
                                    eprintln!(
                                        "Did not find resource -- {:#?}\n{:#?}",
                                        address, self.__internal_resources
                                    );
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    EngineAction::Sync { mut tx } => {
                        info!("Syncing engine handle");
                        if let Some(tx) = tx.take() {
                            if tx.send(self.engine_handle()).is_err() {
                                error!("Could not send updated handle");
                            }
                        }
                    }
                    EngineAction::Shutdown(delay) => {
                        info!(delay_ms = delay.as_millis(), "Shutdown requested");
                        tokio::time::sleep(delay).await;
                        self.cancellation.cancel();
                        break;
                    }
                    #[allow(unreachable_patterns)]
                    _ => {}
                }
            }
        }

        Ok(self)
    }
}

/// Type alias for internal hosted resources,
///
type InternalResources = BTreeMap<Address, HostedResource>;

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
    action: EngineAction,
}

/// Enumeration of actions that can be requested by a packet,
///
#[derive(Serialize, Deserialize)]
enum EngineAction {
    /// Calls a plugin with an assigned address on the engine,
    ///
    Call {
        /// Address of the plugin to call,
        ///
        address: String,
        /// Channel to transmit the result back to the sender,
        ///
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<CallOutput>>,
    },
    /// Gets an updated engine handle,
    ///
    /// **TODO**: This could be used to swap out the internals?
    Sync {
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<EngineHandle>>,
    },
    /// Requests the engine to shutdown,
    ///
    Shutdown(tokio::time::Duration),
}

impl Debug for EngineAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call { address, tx } => f
                .debug_struct("Run")
                .field("address", address)
                .field("has_tx", &tx.is_some())
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
/// An engine handle can also spawn a background task on the tokio runtime which
/// can return an updated engine handle. (Specifically the cache)
///
pub struct EngineHandle {
    /// Sends engine packets to the engine,
    ///
    sender: Arc<tokio::sync::mpsc::UnboundedSender<EnginePacket>>,
    /// Background work engine handle,
    ///
    pub(crate) background_work: Option<BackgroundWorkEngineHandle>,
}

impl Clone for EngineHandle {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            background_work: self.background_work.clone(),
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
        let address = address.into();

        trace!("Looking for {}", &address);
        let (tx, rx) = tokio::sync::oneshot::channel::<CallOutput>();

        let packet = EnginePacket {
            action: EngineAction::Call {
                address: address.into(),
                tx: Some(tx),
            },
        };

        self.sender.send(packet)?;

        match rx.await? {
            CallOutput::Spawn(Some(jh)) => jh.await?,
            CallOutput::Abort(err) => {
                err?;
                Err(anyhow!("Call was aborted"))
            }
            _ => Err(anyhow!("Call was skipped")),
        }
    }

    /// Sends a signal for the engine to shutdown,
    ///
    pub async fn shutdown(&self, delay: tokio::time::Duration) -> anyhow::Result<()> {
        let packet = EnginePacket {
            action: EngineAction::Shutdown(delay),
        };

        self.sender.send(packet)?;
        Ok(())
    }

    /// Synchronize the state of this handle,
    ///
    pub async fn sync(&self) -> anyhow::Result<EngineHandle> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        let packet = EnginePacket {
            action: EngineAction::Sync { tx: Some(tx) },
        };

        self.sender.send(packet)?;

        Ok(rx.await?)
    }

    /// Returns a background work engine handle,
    ///
    pub fn background(&mut self) -> Option<&mut BackgroundWorkEngineHandle> {
        self.background_work.as_mut()
    }
}
