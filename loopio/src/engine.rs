use crate::action::ActionFactory;
use crate::action::HostAction;
use crate::background_work::BackgroundWorkEngineHandle;
use crate::host;
use crate::host::Event;
use crate::operation::Operation;
use crate::prelude::Action;
use crate::prelude::ActionExt;
use crate::prelude::Address;
use crate::prelude::EngineBuildMiddleware;
use crate::prelude::Ext;
use crate::prelude::VirtualBus;
use crate::sequence::Sequence;
use anyhow::anyhow;
use bytes::Bytes;
use futures_util::StreamExt;
use host::Host;
use reality::prelude::*;
use runir::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;

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
    pub(crate) runtime_builder: tokio::runtime::Builder,
    /// Workspace,
    ///
    pub(crate) workspace: Workspace,
}

impl EngineBuilder {
    /// Define an engine build w/ middleware functions
    /// 
    pub fn define(self, middleware: &[EngineBuildMiddleware]) -> EngineBuilder {
        let engine_builder = Engine::builder();

        let engine_builder = middleware.iter().fold(engine_builder, |eb, f| f(eb));

        engine_builder
    }

    /// Creates a new engine builder,
    ///
    pub fn new(runtime_builder: tokio::runtime::Builder) -> Self {
        Self {
            plugins: vec![],
            runtime_builder,
            workspace: EmptyWorkspace.workspace(),
        }
    }

    /// Enables isolation at the runir level by building a new tokio runtime
    /// that is runir entropy aware
    /// 
    pub fn enable_isolation(mut self) -> Self {
        self.runtime_builder = runir::prelude::new_runtime();
        self
    }

    /// Enables a new thunk type that implements the Plugin trait,
    ///
    pub fn enable<P>(&mut self)
    where
        P: Plugin + Default + Clone + ToFrame + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P> + ToOwned<Owned = P>,
    {
        info!("Enabling plugin {}", P::symbol());
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();
            parser.push_link_recv::<P>();
        });
    }

    /// Enables an object type to be parsed,
    ///
    pub fn enable_as<P, Inner>(&mut self)
    where
        P: Plugin + Default + Clone + ToFrame + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = Inner>,
        Inner: Plugin,
        Inner::Virtual: NewFn<Inner = Inner>,
    {
        self.register_with(|parser| {
            let mut block_obj = reality::BlockObjectType::new::<Thunk<Inner>>();

            block_obj.attribute_type = reality::AttributeTypeParser::<Shared>::new_with(
                P::symbol(),
                |parser, input| {
                    P::parse(parser, input);

                    let key = parser
                        .parsed_node
                        .last()
                        .cloned()
                        .unwrap_or(ResourceKey::root());
                    if let Some(storage) = parser.storage() {
                        storage
                            .lazy_put_resource(PluginLevel::new_as::<P, Inner>(), key.transmute());
                    }
                },
                P::link_recv,
                P::link_field,
                ResourceLevel::new::<P>(),
                None,
            );

            parser.add_object_type_with(P::symbol(), block_obj);
        });
    }

    /// Consumes the builder and returns a new engine,
    ///
    pub fn build(mut self) -> Engine {
        #[cfg(feature = "hyper-ext")]
        self.register_with(|p| {
            if let Some(s) = p.storage() {
                let root = s.root_ref();
                root.lazy_put(secure_client());
                root.lazy_put(local_client());
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

    /// Builds the inner tokio runtime, applies builtin plugin utilities and features, and 
    /// compiles the current workspace.
    /// 
    /// Returns a new engine
    /// 
    pub async fn compile(self) -> anyhow::Result<Engine> {
        let workspace = self.workspace.clone();
        let engine = self.build();

        let engine = engine.compile(workspace).await?;
        Ok(engine)
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
/// Plugins are executed as "Thunks" in a "call-by-name" fashion. Plugins belonging to an event share state linearly,
/// meaning after a plugin executes, it can modify state before the next plugin executes.
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
    /// Package
    ///
    pub package: Option<Package>,
    /// Cancelled when the engine is dropped,
    ///
    cancellation: CancellationToken,
    /// Plugins to register w/ the Project
    ///
    plugins: Vec<reality::BlockPlugin<Shared>>,
    /// Engine handle that can be used to send packets to this engine,
    ///
    handle: EngineHandle,
    /// Packet receiver,
    ///
    packet_rx: tokio::sync::mpsc::UnboundedReceiver<EnginePacket>,
    /// Wrapped w/ a runtime so that it can be dropped properly
    ///
    runtime: Option<tokio::runtime::Runtime>,
    /// Remote actions,
    ///
    __remote_actions: Vec<ActionFactory>,
    /// Internal hosted resources,
    ///
    __internal_resources: HostedResourceMap,
    /// Published nodes,
    ///
    __published: BTreeMap<Address, ThunkContext>,
    /// Map of virtual buses,
    ///
    __bus: BTreeMap<Address, VirtualBus>,
}

impl Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO -- Output easier to human parse initialization state

        f.debug_struct("Engine")
            .field("__published", &self.__published.keys())
            .field("__internal_resources", &self.__internal_resources)
            .finish()
    }
}

impl Engine {
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
    pub fn enable<P>(&mut self)
    where
        P: Plugin + Default + Clone + ToFrame + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P>,
    {
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
        let cancellation = CancellationToken::new();
        Engine {
            plugins,
            runtime: Some(runtime),
            cancellation: cancellation.clone(),
            handle: EngineHandle {
                host: None,
                sender: Arc::new(sender),
                background_work: None,
            },
            packet_rx: rx,
            package: None,
            __remote_actions: vec![],
            __internal_resources: BTreeMap::new(),
            __published: BTreeMap::new(),
            __bus: BTreeMap::new(),
        }
    }

    fn add_node_plugin<T>(
        name: Option<&str>,
        tag: Option<&str>,
        parser: &mut AttributeParser<Shared>,
    ) where
        T: Debug + Plugin + BlockObject + crate::prelude::Action + SetIdentifiers,
        T::Virtual: NewFn<Inner = T>,
    {
        let name = name
            .map(|n| n.to_string())
            .unwrap_or(format!("{}", uuid::Uuid::new_v4()));

        T::parse(parser, &name);

        let nk = parser.parsed_node.node.transmute::<T>();
        let node = *parser.parsed_node.last().expect("should have a node level");
        if let Some(mut storage) = parser.storage_mut() {
            storage.drain_dispatch_queues();
            let mut res = storage
                .current_resource(node.transmute::<T>())
                .expect("should exist after T::parse is called");
            trace!("Found node plugin");
            res.bind_node(nk.transmute());
            T::set_identifiers(&mut res, &name, tag.map(|t| t.to_string()).as_ref());

            let mut node = storage.entry(nk.transmute());
            node.put(res);
            node.put(PluginLevel::new::<T>());

            let mut root = storage.root();
            root.put::<ResourceKey<T>>(nk);
        }
        parser.parsed_node.attributes.pop();
        parser.parsed_node.attributes.push(nk.transmute());

        debug!("add_node_plugin\n-----\n{:#?}\n-----", parser.parsed_node);

        parser.push_link_recv::<T>();
    }

    /// Compiles a workspace,
    ///
    pub async fn compile(mut self, workspace: Workspace) -> anyhow::Result<Self> {
        let mut project = Project::new(Shared::default());
        project.add_block_plugin(None, None, |_| {});

        let plugins = self.plugins.clone();
        project.add_node_plugin("operation", move |name, tag, target| {
            let name = name
                .map(|n| n.to_string())
                .unwrap_or(format!("{}", uuid::Uuid::new_v4()));
            let nk = target.parsed_node.node;
            if let Some(mut storage) = target.storage_mut() {
                let mut operation = Operation::new(name, tag.map(|t| t.to_string()));
                operation.bind_node(nk.transmute());

                let mut node = storage.entry(nk.transmute());
                node.put(PluginLevel::new::<Operation>());
                node.put(operation);

                let mut root = storage.root();
                root.put(nk.transmute::<Operation>());
            }
            for p in plugins.iter() {
                p(target);
            }
            target.push_link_recv::<Operation>();
        });

        project.add_node_plugin("sequence", Self::add_node_plugin::<Sequence>);
        project.add_node_plugin("host", Self::add_node_plugin::<Host>);

        let project = workspace.compile(project).await?.project.take().unwrap();
        let package = project.package().await?;

        let contents = package.search("*");
        let mut hosts = vec![];
        for p in contents {
            if let Some(address) = p
                .host
                .address()
                .and_then(|a| Address::from_str(a.as_str()).ok())
            {
                info!("Publishing address -- {}", address);
                let mut context = p.program.context()?;
                context.cancellation = self.cancellation.child_token();
                self.__published.insert(address, context.clone());

                if context.attribute.is_resource::<Host>() {
                    hosts.push(context);
                }
            }
        }

        for mut _host in hosts {
            let mut host = _host.as_remote_plugin::<Host>().await;
            host.bind(_host.clone());

            trace!("Configuring host\n{:#?}", host);
            for a in host.action.iter() {
                if let Some(address) = a.value() {
                    let addr = address.to_string();
                    let resource = self.get_resource(addr).await?;

                    let eh = self.engine_handle().with_host(_host.attribute);

                    resource.context().node().await.root_ref().lazy_put(eh);

                    resource.context().process_node_updates().await;

                    let address = address
                        .clone()
                        .with_host(host.name.value.as_deref().unwrap_or("engine"));

                    // Registers the action to address, can be fetched w/ self.get_resource
                    info!("Registering host action - {}", address);
                    self.__internal_resources.insert(address, resource);
                }
            }

            if let Some(recv) = host.context().attribute.recv() {
                for f in recv.fields().unwrap().iter().cloned() {
                    if f.as_resource()
                        .map(|r| r.is_parse_type::<Event>())
                        .unwrap_or_default()
                    {
                        let host_action = HostAction::new(host.context().attribute);

                        // Upgrade fields into plugins
                        let name = f
                            .as_node()
                            .and_then(|n| n.input())
                            .map(|i| i.to_string())
                            .unwrap_or(f.as_uuid().to_string());

                        let host_action = host_action
                            .build(
                                Event {
                                    name: name.clone(),
                                    data: Bytes::default(),
                                },
                                f,
                            )
                            .await?;

                        self.__remote_actions.push(host_action);
                    }
                }
            }
        }

        self.package = Some(package);

        Ok(self)
    }

    /// Returns a hosted resource,
    ///
    /// First searches internal resources, and then published resources.
    ///
    pub async fn get_resource(&self, address: impl AsRef<str>) -> anyhow::Result<HostedResource> {
        let address: Address = address.as_ref().parse()?;

        if let Some(resource) = self.__internal_resources.get(&address).or(self
            .__published
            .get(&address)
            .map(|tc| tc.get_hosted_resource())
            .as_ref())
        {
            let mut resource = resource.clone();

            // Drain dispatch queues
            {
                let mut node = resource.context_mut().node.storage.write().await;

                let mut root = node.root();
                root.maybe_put(|| self.engine_handle());

                node.drain_dispatch_queues();
            }
            {
                resource
                    .context_mut()
                    .maybe_write_cache(|| self.engine_handle());
            }

            Ok(resource)
        } else {
            Err(anyhow!("Could not find resource: {}", address))
        }
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

    /// Takes ownership of the engine and starts listening for packets,
    ///
    pub fn spawn(
        self,
        middleware: impl Fn(&mut Engine, EnginePacket) -> Option<EnginePacket> + Send + Sync + 'static,
    ) -> (EngineHandle, JoinHandle<anyhow::Result<Self>>) {
        info!("Starting engine packet listener");
        (
            self.engine_handle(),
            tokio::spawn(self.handle_packets(middleware)),
        )
    }

    /// Default start-up procedure,
    ///
    pub async fn default_startup(
        mut self,
    ) -> anyhow::Result<(EngineHandle, JoinHandle<anyhow::Result<Self>>)> {
        let remote_actions = self.__remote_actions.drain(..).collect::<Vec<_>>();
        let startup = self.spawn(|_, p| {
            trace!("{:?}", p);
            Some(p)
        });

        let eh = startup.0.clone();

        // Publish all remote actions
        for ra in remote_actions {
            let published = ra.publish(eh.clone()).await?;
            for p in published {
                info!("Default startup published remote action - {}", p);
            }
        }

        Ok(startup)
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
                        trace!(address, "Looking up hosted resource");
                        if let Some(tx) = tx.take() {
                            if let Ok(resource) = self.get_resource(address).await {
                                trace!("Sending call output");
                                if tx.send(resource.spawn()).is_err() {
                                    error!("Could not call resource");
                                }
                            } else {
                                drop(tx);
                            }
                        }
                    }
                    EngineAction::Resource { address, mut tx } => {
                        trace!(address, "Looking up hosted resource");

                        if let Some(tx) = tx.take() {
                            if let Ok(mut resource) = self.get_resource(address).await {
                                let mut published = self
                                    .__published
                                    .keys()
                                    .map(|a| a.to_string())
                                    .collect::<Vec<_>>();
                                published.append(
                                    &mut self
                                        .__internal_resources
                                        .keys()
                                        .map(|a| a.to_string())
                                        .collect::<Vec<_>>(),
                                );

                                let published = Published {
                                    label: String::new(),
                                    resources: published
                                        .iter()
                                        .filter_map(|a| Decorated::from_str(a).ok())
                                        .collect(),
                                };

                                {
                                    resource.context_mut().write_cache(published);
                                }

                                if tx.send(Some(resource)).is_err() {
                                    error!("Could not call resource");
                                }
                                continue;
                            }

                            if tx.send(None).is_err() {
                                eprintln!("Could not send spawn result");
                            }
                        }
                    }
                    EngineAction::Publish { context, mut tx } => {
                        if let Some(tx) = tx.take() {
                            trace!("Looking up address");
                            let address = context.address();

                            trace!("Got address {address}");
                            if let Ok(address) = address.parse::<Address>() {
                                trace!("Publishing hosted resource {address}");

                                if let Some(mut filter) = address.filter() {
                                    if let Some((_, event)) =
                                        filter.find(|(k, _)| k == Event::symbol())
                                    {
                                        if !self.__bus.contains_key(&address) {
                                            info!("Detected event publish, registering virtual bus -- {} {}", event, address);
                                            let bus = VirtualBus::from(context.clone());

                                            self.__bus.insert(address.clone(), bus);
                                        }
                                    }
                                }

                                if !self.__internal_resources.contains_key(&address)
                                    && !self.__published.contains_key(&address)
                                {
                                    self.__published.insert(address.clone(), context);

                                    if tx.send(Ok(address)).is_err() {
                                        error!("Could not publish resource");
                                    }
                                } else if tx
                                    .send(Err(anyhow!(
                                        "Could not publish {address}, already occupied"
                                    )))
                                    .is_err()
                                {
                                    error!("Could not publish resource");
                                }
                            } else if tx.send(Err(anyhow!("Could not parse {address}"))).is_err() {
                                error!("Could not publish resource");
                            }
                        } else {
                            panic!("Expected a response channel")
                        }
                    }
                    EngineAction::Sync { mut tx } => {
                        trace!("Syncing engine handle");
                        if let Some(tx) = tx.take() {
                            if tx.send(self.engine_handle()).is_err() {
                                error!("Could not send updated handle");
                            }
                        }
                    }
                    EngineAction::Bus { address, mut tx } => {
                        if let Some(tx) = tx.take() {
                            if let Some(bus) = self.__bus.get(&address) {
                                if tx.send(bus.clone()).is_err() {
                                    error!("Could not send bus");
                                }
                            }
                        }
                    }
                    EngineAction::Shutdown(delay) => {
                        warn!(delay_ms = delay.as_millis(), "Shutdown requested");
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

/// List of all published addresses hosted on an engine,
///
#[derive(Reality, Default, Clone, Debug)]
#[plugin_def(
    call = build_published
)]
pub struct Published {
    /// Label for this list,
    ///
    #[reality(derive_fromstr)]
    pub label: String,
    /// List of resources that have been published,
    ///
    #[reality(vec_of=Decorated<Address>)]
    pub resources: Vec<Decorated<Address>>,
}

async fn build_published(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let eh = tc.engine_handle().await;

    if let Some(eh) = eh {
        let mut _a = eh.hosted_resource("engine://engine").await?;

        if let Some(published) = _a.context().cached::<Published>() {
            let mut transient = tc.transient_mut().await;
            published.clone().pack(transient.deref_mut());

            transient.root().put(published.clone());
        };
    }

    Ok(())
}

/// Type alias for internal hosted resources,
///
type HostedResourceMap = BTreeMap<Address, HostedResource>;

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
    /// Retrieves a hosted resource,
    ///
    Resource {
        /// Address of the hosted resource to retrieve,
        ///
        address: String,
        /// Channel to transmit the hosted resource if found,
        ///
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<Option<HostedResource>>>,
    },
    /// Publishes a new thunk context as a node,
    ///
    Publish {
        /// Address to publish the transient storage to,
        ///
        #[serde(skip)]
        context: ThunkContext,
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<anyhow::Result<Address>>>,
    },
    /// Gets an updated engine handle,
    ///
    Sync {
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<EngineHandle>>,
    },
    Bus {
        /// Bus address
        ///
        address: Address,
        #[serde(skip)]
        tx: Option<tokio::sync::oneshot::Sender<VirtualBus>>,
    },
    /// Requests the engine to shutdown,
    ///
    Shutdown(tokio::time::Duration),
}

impl Debug for EngineAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call { address, tx } => f
                .debug_struct("Call")
                .field("address", address)
                .field("has_tx", &tx.is_some())
                .finish(),
            Self::Resource { address, tx } => f
                .debug_struct("Resource")
                .field("address", address)
                .field("has_tx", &tx.is_some())
                .finish(),
            Self::Sync { tx } => f
                .debug_struct("Sync")
                .field("has_tx", &tx.is_some())
                .finish(),
            Self::Bus { address, tx } => f
                .debug_struct("Bus")
                .field("address", &address.to_string())
                .field("has_tx", &tx.is_some())
                .finish(),
            Self::Shutdown(arg0) => f.debug_tuple("Shutdown").field(arg0).finish(),
            Self::Publish { tx, .. } => f
                .debug_struct("Publish")
                .field("has_tx", &tx.is_some())
                .finish(),
            #[allow(unreachable_patterns)]
            _ => Ok(()),
        }
    }
}

/// Handle for communicating and sending work packets to an engine,
///
/// An engine handle can also spawn a background task on the tokio runtime which
/// can return an updated engine handle. (Specifically the cache)
///
pub struct EngineHandle {
    /// Host this handle is attached to,
    ///
    pub host: Option<ResourceKey<Attribute>>,
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
            host: self.host,
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
    /// Returns the handle w/ host set,
    ///
    pub fn with_host(mut self, host: ResourceKey<Attribute>) -> Self {
        self.host = Some(host);
        self
    }

    /// Runs an operation by sending a packet and waits for a response,
    ///
    pub async fn run(&self, address: impl Into<String>) -> anyhow::Result<ThunkContext> {
        let address = address.into();

        debug!("Looking for {}", &address);
        let (tx, rx) = tokio::sync::oneshot::channel::<CallOutput>();

        let packet = EnginePacket {
            action: EngineAction::Call {
                address,
                tx: Some(tx),
            },
        };

        self.sender.send(packet)?;

        match rx.await? {
            CallOutput::Spawn(Some(jh)) => {
                trace!("Spawning update");
                jh.await?
            }
            CallOutput::Abort(err) => {
                err?;
                Err(anyhow!("Call was aborted"))
            }
            CallOutput::Update(Some(next)) => Ok(next),
            _ => Err(anyhow!("Call was skipped")),
        }
    }

    /// Retrieves a hosted resource,
    ///
    /// TODO: Need to return an error if no thread is running to handle packets
    ///
    pub async fn hosted_resource(
        &self,
        address: impl Into<String>,
    ) -> anyhow::Result<HostedResource> {
        let address = address.into();

        trace!("Looking for {}", &address);
        let (tx, rx) = tokio::sync::oneshot::channel::<Option<HostedResource>>();

        let packet = EnginePacket {
            action: EngineAction::Resource {
                address: address.to_string(),
                tx: Some(tx),
            },
        };

        self.sender.send(packet)?;

        match rx.await? {
            Some(resource) => Ok(resource),
            None => Err(anyhow::anyhow!("Could not find resource {address}")),
        }
    }

    /// Publish a resource on the engine,
    ///
    pub async fn publish(&self, context: ThunkContext) -> anyhow::Result<Address> {
        trace!("Trying to publish {}", &context.address());
        let (tx, rx) = tokio::sync::oneshot::channel::<anyhow::Result<Address>>();

        let packet = EnginePacket {
            action: EngineAction::Publish {
                context,
                tx: Some(tx),
            },
        };

        self.sender.send(packet)?;

        rx.await?
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

    /// Returns a virtual bus for some event,
    ///
    pub(crate) async fn event_vbus(&self, host: &str, name: &str) -> anyhow::Result<VirtualBus> {
        let address: Address = format!("{host}?{}={name}", Event::symbol()).parse()?;

        debug!("Looking for event vbus {}", address);

        let (tx, rx) = tokio::sync::oneshot::channel();

        let packet = EnginePacket {
            action: EngineAction::Bus {
                address,
                tx: Some(tx),
            },
        };

        self.sender.send(packet)?;

        Ok(rx.await?)
    }

    /// Listens for an event,
    ///
    pub(crate) async fn listen(&self, event: impl AsRef<str>) -> anyhow::Result<Option<Bytes>> {
        let host = self
            .host
            .and_then(|h| h.address().as_deref().cloned())
            .unwrap_or(String::from("engine"));

        match self.event_vbus(&host, event.as_ref()).await {
            Ok(mut vbus) => {
                let next = vbus.wait_for::<Event>().await;

                let mut next = next.select(|n| &n.virtual_ref().name);
                let mut port = futures_util::StreamExt::boxed(&mut next);

                info!("Listening for event -- {}", event.as_ref());

                let mut message = None::<Bytes>;
                if let Some((_, event)) = port.next().await {
                    info!("Got event -- {:?}", event);

                    if !event.data.is_empty() {
                        message = Some(event.data);
                    }
                }

                info!("Finished listening");
                Ok(message)
            }
            Err(err) => {
                error!("Could not listen for event {err}");
                Err(err)
            }
        }
    }

    /// Notifies listeners of an event,
    ///
    pub(crate) async fn notify(
        &self,
        event: impl AsRef<str>,
        data: Option<Bytes>,
    ) -> anyhow::Result<()> {
        let host = self
            .host
            .and_then(|h| h.address().as_deref().cloned())
            .unwrap_or(String::from("engine"));

        match self.event_vbus(&host, event.as_ref()).await {
            Ok(mut vbus) => {
                let writer = vbus.transmit::<Event>().await;

                writer.write_to_virtual(move |r| {
                    if let Some(data) = data {
                        r.virtual_ref().send_raw().send_if_modified(|o| {
                            o.data = data;
                            true
                        });
                    }
                    r.virtual_mut().name.commit()
                });

                Ok(())
            }
            Err(err) => {
                error!("Could not listen for event {err}");
                Err(err)
            }
        }
    }
}
