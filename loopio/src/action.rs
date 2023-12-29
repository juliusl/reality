use anyhow::anyhow;
use async_trait::async_trait;
use futures_util::Future;
use std::pin::Pin;
use tracing::debug;
use uuid::Uuid;

use reality::attributes;
use reality::attributes::Node;

use crate::prelude::*;

/// Common trait for engine node types,
///
pub trait Action {
    /// Return the address of an action,
    ///
    fn address(&self) -> String;

    /// Bind a thunk context to the action,
    ///
    /// **Note** This context has access to the compiled node this action corresponds to.
    ///
    fn bind(&mut self, context: ThunkContext);

    /// Binds the node attribute's resource key to this action,
    ///
    fn bind_node(&mut self, node: ResourceKey<attributes::Node>);

    /// Binds a plugin to this action's plugin resource key,
    ///
    /// **Note** If not set, then the default is the default plugin key.
    ///
    fn bind_plugin(&mut self, plugin: ResourceKey<attributes::Attribute>);

    /// Returns the bound node resource key for this action,
    ///
    fn node_rk(&self) -> ResourceKey<attributes::Node>;

    /// Returns the plugin fn resource key for this action,
    ///
    fn plugin_rk(&self) -> ResourceKey<attributes::Attribute>;

    /// Returns the current context,
    ///
    /// **Note** Should panic if currently unbound,
    ///
    fn context(&self) -> &ThunkContext;

    /// Returns a mutable reference to the current context,
    ///
    /// **Note** Should panic if currently unbound,
    ///
    fn context_mut(&mut self) -> &mut ThunkContext;

    /// Spawns the thunk attached to the current context for this action,
    ///
    fn spawn(&self) -> SpawnResult
    where
        Self: CallAsync,
    {
        self.context().spawn(|mut tc| async move {
            <Self as CallAsync>::call(&mut tc).await?;
            Ok(tc)
        })
    }

    /// Returns a future that contains the result of the action,
    ///
    fn spawn_call(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<ThunkContext>> + Send + '_>>
    where
        Self: Sync,
    {
        Box::pin(async move {
            let r = self.into_hosted_resource();
            if let Some(s) = r.spawn() {
                if let Ok(s) = s.await {
                    s
                } else {
                    Err(anyhow!("Task could not join"))
                }
            } else {
                Err(anyhow!("Did not spawn a a task"))
            }
        })
    }

    /// Convert the action into a generic hosted resource,
    ///
    fn into_hosted_resource(&self) -> HostedResource {
        HostedResource {
            address: self.address(),
            node_rk: self.node_rk(),
            rk: self.plugin_rk(),
            binding: Some(self.context().clone()),
        }
    }

    /// Converts a pointer to the hosted resource into call output,
    ///
    fn into_call_output(&self) -> CallOutput {
        CallOutput::Spawn(self.into_hosted_resource().spawn())
    }
}

#[async_trait]
pub trait ActionExt: Action + Send + Sync {
    /// Returns the simple form of the plugin,
    ///
    /// **Note** The simple form only initializes from runmd instructions.
    ///
    #[inline]
    async fn as_plugin<P>(&self) -> P
    where
        P: Plugin,
    {
        self.context().initialized::<P>().await
    }

    /// Returns the remote plugin form of the plugin,
    ///
    #[inline]
    async fn as_remote_plugin<P>(&mut self) -> P
    where
        Self: Sync,
        P: Plugin,
    {
        Remote.create(self.context_mut()).await
    }

    /// Returns the local plugin form of the plugin,
    ///
    #[inline]
    async fn as_local_plugin<P>(&mut self) -> P
    where
        P: Plugin,
    {
        Local.create(self.context_mut()).await
    }

    /// Returns as a dispatcher for some resource R,
    ///
    /// **Note** -- Dispatches any pending messages before returning the dispatcher.
    ///
    #[inline]
    async fn as_dispatch<R>(&self) -> Dispatcher<Shared, R>
    where
        R: Default + Send + Sync + 'static,
    {
        let mut disp = self.context().dispatcher::<R>().await;
        disp.dispatch_all().await;
        disp
    }
}

#[async_trait]
impl ActionExt for Host {}
#[async_trait]
impl ActionExt for Sequence {}
#[async_trait]
impl ActionExt for Operation {}
#[async_trait]
impl ActionExt for HostedResource {}

impl Action for HostedResource {
    #[inline]
    fn address(&self) -> String {
        self.address.to_string()
    }

    #[inline]
    fn bind(&mut self, context: ThunkContext) {
        self.binding = Some(context);
    }

    #[inline]
    fn context(&self) -> &ThunkContext {
        self.binding.as_ref().expect("should be bound to an engine")
    }

    #[inline]
    fn context_mut(&mut self) -> &mut ThunkContext {
        self.binding.as_mut().expect("should be bound to an engine")
    }

    #[inline]
    fn bind_node(&mut self, node: ResourceKey<Node>) {
        self.node_rk = node;
    }

    #[inline]
    fn node_rk(&self) -> ResourceKey<Node> {
        self.node_rk
    }

    #[inline]
    fn plugin_rk(&self) -> ResourceKey<Attribute> {
        self.rk
    }

    #[inline]
    fn bind_plugin(&mut self, plugin: ResourceKey<reality::attributes::Attribute>) {
        self.rk = plugin;
    }
}

impl Action for ThunkContext {
    #[inline]
    fn address(&self) -> String {
        self.property("address")
            .map(|s| s.to_string())
            .unwrap_or(self.variant_id.unwrap_or(Uuid::new_v4()).to_string())
    }

    #[inline]
    fn bind(&mut self, context: ThunkContext) {
        *self = context;
    }

    #[inline]
    fn bind_node(&mut self, node: ResourceKey<attributes::Node>) {
        self.write_cache(node)
    }

    #[inline]
    fn bind_plugin(&mut self, plugin: ResourceKey<attributes::Attribute>) {
        self.attribute = plugin;
    }

    #[inline]
    fn node_rk(&self) -> ResourceKey<attributes::Node> {
        self.cached().unwrap_or_default()
    }

    #[inline]
    fn plugin_rk(&self) -> ResourceKey<attributes::Attribute> {
        self.attribute
    }

    #[inline]
    fn context(&self) -> &ThunkContext {
        self
    }

    #[inline]
    fn context_mut(&mut self) -> &mut ThunkContext {
        self
    }
}

#[async_trait]
impl ActionExt for ThunkContext {}

#[tokio::test]
async fn test_thunk_context_action() {
    let (uuid, mut tc) = ThunkContext::new().branch();
    let rk = ResourceKey::with_hash("test");
    tc.bind_plugin(rk);

    let node = tc.node().await;
    node.lazy_put_resource::<ThunkFn>(
        |tc| {
            CallOutput::Spawn(tc.spawn(|tc| async move {
                eprintln!("hello world");
                Ok(tc)
            }))
        },
        rk.transmute(),
    );

    tc.node.storage.write().await.drain_dispatch_queues();

    let r = tc.into_hosted_resource();
    assert_eq!(r.address(), uuid.to_string());
    assert_eq!(r.plugin_rk(), rk);
    let _ = r.spawn_call().await.unwrap(); // Will panic if the thunk fn was not called
    ()
}

/// Pointer-struct for creating local actions,
///
pub struct LocalAction;

impl LocalAction {
    /// Prepares a new local action builder,
    ///
    pub async fn build<P>(self, context: &mut ThunkContext) -> ActionFactory
    where
        P: Plugin,
        P::Virtual: NewFn<Inner = P>,
    {
        let inner = context.as_local_plugin::<P>().await;
        let transient = context.transient_mut().await;

        drop(transient);

        ActionFactory {
            attribute: context.attribute,
            storage: context.transient.clone(),
            address: None,
        }
        .set_entrypoint(inner)
    }
}

/// Pointer-struct for creating remote action builder,
///
pub struct RemoteAction;

impl RemoteAction {
    /// Prepares a new remote action builder,
    ///
    pub async fn build<P>(self, context: &mut ThunkContext) -> ActionFactory
    where
        P: Plugin,
        P::Virtual: NewFn<Inner = P>,
    {
        let inner = context.as_remote_plugin::<P>().await;

        let mut transient = context.transient_mut().await;

        // If enabled, allows available field packets to be decoded,
        {
            let node = context.node().await;
            if let Some(bus) = node.current_resource::<WireBus>(context.attribute.transmute()) {
                debug!("Found wire bus");
                drop(node);
                transient.put_resource(bus, context.attribute.transmute());
            }
        }

        // If set, allows the ability to apply frame updates. (**Note** The receiving end must enable updating)
        {
            let node = context.node().await;
            if let Some(change_pipeline) =
                node.current_resource::<FrameUpdates>(context.attribute.transmute())
            {
                debug!("Found frame updates");
                drop(node);
                transient.put_resource(change_pipeline, context.attribute.transmute());
            }
        }

        // Get the receiver from the frame to find any decorations
        let recv = context.initialized_frame().await.recv.clone();

        // Gets the current parsed attributes state of the target attribute,
        {
            let node = context.node().await;
            if let Some(parsed_node) = node.current_resource::<ParsedNode>(ResourceKey::root()) {
                drop(node);
                transient.put_resource(parsed_node, ResourceKey::root());
                transient.put_resource(recv, context.attribute.transmute());
            }
        }

        drop(transient);

        ActionFactory {
            attribute: context.attribute,
            storage: context.transient.clone(),
            address: None,
        }
        .set_entrypoint(inner)
    }
}

/// Action factory,
///
pub struct ActionFactory {
    /// Resource key for this action,
    ///
    pub attribute: ResourceKey<Attribute>,
    /// Thunk context to build action components,
    ///
    pub storage: AsyncStorageTarget<Shared>,
    /// Optional address to publish this action to,
    ///
    address: Option<Address>,
}

/// Type-alias for a task future,
///
type Task = Pin<Box<dyn Future<Output = anyhow::Result<ThunkContext>> + Send + Sync + 'static>>;

/// Type-alias for a task fn resource,
///
type TaskFn = Pin<Box<dyn Fn(ThunkContext) -> Task + Send + Sync + 'static>>;

impl ActionFactory {
    /// Sets the current address,
    ///
    pub fn set_address(mut self, address: Address) -> Self {
        self.address = Some(address);
        self
    }

    /// Sets the entrypoint for the action,
    ///
    pub fn set_entrypoint<P>(self, plugin: P) -> Self
    where
        P: Plugin,
        P::Virtual: NewFn<Inner = P>,
    {
        let mut storage = self
            .storage
            .storage
            .try_write()
            .expect("should be able to write");

        let key = self.attribute().transmute();

        storage.put_resource::<P>(plugin, key);

        drop(storage);
        self
    }

    /// Registers a plugin call to a symbol,
    ///
    pub fn enable<P>(self, plugin: P) -> Self
    where
        P: Plugin,
        P::Virtual: NewFn<Inner = P>,
    {
        let key = self.attribute().branch(P::symbol());

        let mut storage = self
            .storage
            .storage
            .try_write()
            .expect("should be able to write");
        storage.put_resource::<P>(plugin, key.transmute());
        storage.put_resource::<ThunkFn>(<P as Plugin>::call, key.transmute());
        storage.put_resource::<EnableFrame>(
            EnableFrame(<P as Plugin>::enable_frame),
            self.attribute().transmute(),
        );
        storage.put_resource::<EnableVirtual>(
            EnableVirtual(<P as Plugin>::enable_virtual),
            self.attribute().transmute(),
        );
        if let Some(mut attrs) = storage.resource_mut::<ParsedNode>(ResourceKey::root()) {
            attrs.attributes.push(self.attribute());
        }
        storage.put_resource(self.attribute(), ResourceKey::default());

        drop(storage);
        self
    }

    /// Binds a task to the action context being built,
    ///
    pub fn bind_task<F>(
        self,
        symbol: &str,
        task: impl Fn(ThunkContext) -> F + Copy + Sync + Send + 'static,
    ) -> Self
    where
        Self: Sync,
        F: Future<Output = anyhow::Result<ThunkContext>> + Sync + Send + 'static,
    {
        let key = self.attribute().branch(symbol);

        let mut storage = self
            .storage
            .storage
            .try_write()
            .expect("should be able to write");

        storage.put_resource(task, key.transmute());
        storage.put_resource::<TaskFn>(
            Box::pin(move |tc| Box::pin(async move { task(tc).await })),
            key.transmute(),
        );
        drop(storage);

        self
    }

    /// Binds a function to a symbol,
    ///
    pub fn bind(self, symbol: &str, plugin: fn(ThunkContext) -> CallOutput) -> Self {
        let key = self.attribute().branch(symbol);

        let mut storage = self
            .storage
            .storage
            .try_write()
            .expect("should be able to write");
        storage.put_resource::<ThunkFn>(plugin, key.transmute());
        drop(storage);
        self
    }

    /// Publishes this factory,
    ///
    pub async fn publish(self, eh: EngineHandle) -> anyhow::Result<Address> {
        let mut tc: ThunkContext = self.storage.into();
        tc.set_attribute(self.attribute);

        if let Some(address) = self.address.as_ref() {
            tc.set_property("address", address.to_string())?;
        }

        eh.publish(tc).await
    }

    /// Publishes this factory and returns the hosted resource,
    ///
    pub async fn publish_hosted_resource(self, eh: EngineHandle) -> anyhow::Result<HostedResource> {
        if let Ok(address) = self.publish(eh.clone()).await {
            eh.hosted_resource(address.to_string()).await
        } else {
            Err(anyhow!("Could not publish action factory"))
        }
    }

    fn attribute(&self) -> ResourceKey<Attribute> {
        self.attribute
    }
}

impl From<ActionFactory> for ThunkContext {
    fn from(value: ActionFactory) -> Self {
        value
            .storage
            .storage
            .try_write()
            .expect("should be able to write")
            .put_resource(value.attribute, ResourceKey::root());
        value.storage.into()
    }
}

/// Trait for trying to call a thunk by name w/ a thunk context,
///
#[async_trait]
pub trait TryCallExt: AsRef<ThunkContext> {
    /// Try calling a thunk by symbol,
    ///
    async fn try_call(&self, symbol: &str) -> anyhow::Result<Option<ThunkContext>> {
        let mut node = self.as_ref().node.storage.write().await;
        let key = self.as_ref().attribute.branch(symbol);

        let mut tc = self.as_ref().clone();
        tc.reset();

        // tc.decoration = node.take_resource::<Decoration>(ResourceKey::root()).map(|d| *d);
        // eprintln!("{:?}", tc.decoration);

        if let Some(_thunk) = node.resource::<ThunkFn>(key.transmute()) {
            let thunk = _thunk.clone();
            drop(_thunk);

            return match thunk(tc) {
                CallOutput::Spawn(Some(op)) => Ok(op.await?.ok()),
                CallOutput::Abort(r) => Err(r.expect_err("should be an error returned here")),
                CallOutput::Update(u) => Ok(u),
                _ => {
                    // Skipping
                    Ok(None)
                }
            };
        }

        if let Some(taskfn) = node.take_resource::<TaskFn>(key.transmute()) {
            drop(node);
            let result = taskfn(tc).await.map(Some);
            return result;
        }

        Ok(None)
    }
}

impl TryCallExt for ThunkContext {}
impl TryCallExt for Host {}
impl TryCallExt for Sequence {}
impl TryCallExt for Operation {}
impl TryCallExt for HostedResource {}

impl AsRef<ThunkContext> for Host {
    fn as_ref(&self) -> &ThunkContext {
        self.context()
    }
}

impl AsRef<ThunkContext> for Sequence {
    fn as_ref(&self) -> &ThunkContext {
        self.context()
    }
}

impl AsRef<ThunkContext> for Operation {
    fn as_ref(&self) -> &ThunkContext {
        self.context()
    }
}

#[tokio::test]
#[tracing_test::traced_test]
async fn test_custom_action() {
    let builder = Engine::builder().define(&[|mut eb| {
        eb.enable::<CustomAction>();

        eb.workspace_mut().add_buffer(
            "test-custom-action.md",
            r#"
            ```runmd
            + .operation a
            <test.custom-action>     test_action
            |# address      =       test://custom-action
            ```
            "#,
        );
        eb
    }]);

    let _engine = builder.compile().await.unwrap();
    eprintln!("{:#?}", _engine);

    let (eh, _) = _engine.spawn(|_, p| Some(p));
    let _tc = eh.run("engine://a").await.unwrap();
    ()
}

#[derive(Reality, Default, Clone)]
#[reality(call = custom_action, plugin, rename = "custom-action", group = "test")]
struct CustomAction {
    #[reality(derive_fromstr)]
    name: String,
}

/// Example of bootstrapping resources,
///
async fn custom_action(_tc: &mut ThunkContext) -> anyhow::Result<()> {
    eprintln!("custom action init");

    // Create the local action
    let action = LocalAction
        .build::<Host>(_tc)
        .await
        // Add a task
        .bind_task("test 123", |mut tc| async move {
            eprintln!("test 123");

            tc.take_cache::<usize>();

            Ok(tc)
        });

    // Publish and retrieve the hosted resource
    let local_action = action
        .publish_hosted_resource(
            _tc.engine_handle()
                .await
                .expect("should be bound to an engine"),
        )
        .await?;

    // Call the entry point
    eprintln!("{}", local_action.address());
    let __la = local_action.spawn_call().await?;

    // Try to call a registered task or plugin on this action
    local_action
        .context()
        .try_call("test 123")
        .await?
        .expect("should have call");

    if let Some(mut show) = local_action.context().try_call("show_ui_node").await? {
        if let Some(__ui_node) = show.take_cache::<()>() {}
    }

    let _host = local_action.as_plugin::<Host>().await;

    Ok(())
}
