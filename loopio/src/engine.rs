use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::DerefMut;
use std::sync::Arc;

use reality::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

use anyhow::anyhow;
use tracing::info;

use crate::operation::Operation;
use crate::plugin::Plugin;
use crate::plugin::Thunk;
use crate::plugin::ThunkContext;
use crate::sequence::Sequence;

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
    pub fn register<P: Plugin + Send + Sync + 'static>(&mut self) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn register_extension<
        C: ExtensionController<P> + Send + Sync + 'static,
        P: Plugin + Send + Sync + 'static,
    >(
        &mut self,
    ) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<ExtensionPlugin<C, P>>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    #[inline]
    pub fn register_with(&mut self, plugin: fn(&mut AttributeParser<Shared>)) {
        self.plugins.push(Arc::new(plugin));
    }

    /// Consumes the builder and returns a new engine,
    ///
    pub fn build(mut self) -> Engine {
        let runtime = self.runtime_builder.build().unwrap();

        Engine::new_with(self.plugins, runtime)
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
}

impl Engine {
    /// Creates a new engine builder,
    ///
    pub fn builder() -> EngineBuilder {
        EngineBuilder::new(tokio::runtime::Builder::new_multi_thread())
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn register<P: Plugin + Send + Sync + 'static>(&mut self) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn register_extension<
        C: ExtensionController<P> + Send + Sync + 'static,
        P: Plugin + Send + Sync + 'static,
    >(
        &mut self,
    ) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<ExtensionPlugin<C, P>>>();
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

        Engine {
            plugins,
            runtime: Some(runtime),
            cancellation: CancellationToken::new(),
            operations: BTreeMap::new(),
            sequences: BTreeMap::new(),
            handle: EngineHandle {
                sender: Arc::new(sender),
            },
            packet_rx: rx,
        }
    }

    /// Creates a new context on this engine,
    ///
    /// **Note** Each time a thunk context is created a new output storage target is generated, however the original storage target is used.
    ///
    pub fn new_context(&self, storage: Arc<tokio::sync::RwLock<Shared>>) -> ThunkContext {
        let mut context = ThunkContext::from(AsyncStorageTarget::from_parts(
            storage,
            self.runtime
                .as_ref()
                .map(|r| r.handle().clone())
                .expect("should have a runtime"),
        ));
        context.engine_handle = Some(self.engine_handle());
        context.cancellation = self.cancellation.child_token();
        context
    }

    /// Compiles operations from the parsed project,
    ///
    pub async fn compile(mut self, workspace: Workspace) -> Self {
        use std::ops::Deref;

        let mut project = Project::new(Shared::default());
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

        if let Some(project) = workspace
            .compile(project)
            .await
            .ok()
            .and_then(|mut w| w.project.take())
        {
            let nodes = project.nodes.into_inner().unwrap();

            for (_, target) in nodes.iter() {
                if let Some(operation) = target.read().await.resource::<Operation>(None) {
                    let mut operation = operation.deref().clone();
                    operation.bind(self.new_context(target.clone()));

                    if let Some(previous) = self.operations.insert(operation.address(), operation) {
                        info!(address = previous.address(), "Replacing operation");
                    }
                }

                let seqkey = target
                    .read()
                    .await
                    .resource::<ResourceKey<Sequence>>(None)
                    .as_deref()
                    .cloned();
                if let Some(sequence) = target.read().await.resource::<Sequence>(seqkey) {
                    if let Some(previous) = self.sequences.insert(sequence.address(), sequence.bind(self.new_context(target.clone()))) {
                        info!(address = previous.address(), "Replacing sequence");
                    }
                }
            }
        }

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
        self.handle.clone()
    }

    /// Starts handling engine packets,
    ///
    pub async fn handle_packets(mut self) -> Self {
        while let Some(packet) = self.packet_rx.recv().await {
            info!("Handling packet");
            match packet.action {
                Action::Run { address, tx } => {
                    info!(address, "Running operation");
                    let result = self.run(address).await;

                    // TODO: Add a way to queue up failed packets?
                    if let Some(tx) = tx {
                        let _ = tx.send(result);
                    }
                }
                Action::Compile(_) => {
                    info!("Compiling content");
                    // self.load_source(content).await;
                    // self.compile().await;
                }
                Action::Shutdown(delay) => {
                    info!(delay_ms = delay.as_millis(), "Shutdown requested");
                    tokio::time::sleep(delay).await;
                    self.cancellation.cancel();
                    break;
                }
            }
        }

        self
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
        address: String,
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<anyhow::Result<ThunkContext>>>,
    },
    /// Compiles the operations from a project,
    ///
    Compile(String),
    /// Requests the engine to
    ///
    Shutdown(tokio::time::Duration),
}

/// Handle for communicating and sending work packets to an engine,
///
#[derive(Clone)]
pub struct EngineHandle {
    /// Sends engine packets to the engine,
    ///
    sender: Arc<tokio::sync::mpsc::UnboundedSender<EnginePacket>>,
}

impl Debug for EngineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineHandle").field("sender", &self.sender).finish()
    }
}

impl EngineHandle {
    /// Runs an operation by sending a packet and waits for a response,
    ///
    pub async fn run(&self, address: impl Into<String>) -> anyhow::Result<ThunkContext> {
        let (tx, rx) = tokio::sync::oneshot::channel::<anyhow::Result<ThunkContext>>();

        let packet = EnginePacket {
            action: Action::Run {
                address: address.into(),
                tx: Some(tx),
            },
        };

        self.sender.send(packet)?;

        rx.await?
    }

    /// Compiles content,
    ///
    pub async fn compile(&self, content: impl Into<String>) -> anyhow::Result<()> {
        let packet = EnginePacket {
            action: Action::Compile(content.into()),
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
}
