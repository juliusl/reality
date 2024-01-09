use async_trait::async_trait;
use futures_util::Future;
use reality::prelude::runir::prelude::CrcInterner;
use reality::prelude::runir::prelude::HostLevel;
use reality::prelude::runir::prelude::Repr;
use serde::Deserialize;
use serde::Serialize;
use std::pin::Pin;
use tracing::info;
use tracing::trace;
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
    fn spawn(&self) -> CallOutput
    where
        Self: CallAsync,
    {
        self.context().spawn(|mut tc| async move {
            <Self as CallAsync>::call(&mut tc).await?;
            Ok(tc)
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

/// Point-struct for creating host actions,
///
pub struct HostAction {
    /// Thunk context for the host hosting this action
    ///
    host: ResourceKey<Attribute>,
}

impl HostAction {
    /// Creates a new host action,
    ///
    pub fn new(host: ResourceKey<Attribute>) -> Self {
        Self { host }
    }

    /// Get the host name,
    ///
    fn host_name(&self) -> Option<String> {
        self.host
            .host()
            .and_then(|h| h.address())
            .map(|a| a.to_string())
    }

    /// Build the host actoin and return the parent,
    ///
    pub async fn build<P>(self, plugin: P, mut repr: Repr) -> anyhow::Result<ActionFactory>
    where
        P: Plugin,
        P::Virtual: NewFn<Inner = P>,
    {
        if let Some(host_name) = self.host_name() {
            let parsed_node_repr = repr.clone();

            // Upgrade fields into plugins
            let name = repr
                .as_node()
                .and_then(|n| n.input())
                .map(|i| i.to_string())
                .unwrap_or(repr.as_uuid().to_string());

            let plugin_symbol = P::symbol().to_lowercase();

            let h = HostLevel::new(format!("{}?{plugin_symbol}={name}", host_name));
            repr.upgrade(runir::prelude::CrcInterner::default(), h)
                .await?;

            let p = PluginLevel::new::<P>();
            repr.upgrade(runir::prelude::CrcInterner::default(), p)
                .await?;

            let key = ResourceKey::<P>::with_repr(repr);
            info!("Host action resource key is -- {:?} {}", key, host_name);

            // Reconstruct a parsed node
            let mut node = ParsedNode::default();
            node.node = self.host;
            node.attributes.push(key.transmute());

            let mut tc = ThunkContext::new();
            tc.set_attribute(key.transmute());
            {
                let mut _node = tc.node.storage.write().await;
                _node.put_resource(plugin, key.transmute());
                _node.put_resource(node, ResourceKey::root());
                _node.put_resource::<ResourceKey<P>>(key, ResourceKey::root());
            }

            let address =
                Address::from_str(format!("{}?{plugin_symbol}={name}", host_name).as_str())?;

            let action = ActionFactory {
                attribute: key.transmute(),
                storage: tc.node.clone(),
                address: Some(address),
                parsed_node_repr,
            };

            Ok(action)
        } else {
            Err(anyhow::anyhow!("Missing host name"))
        }
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
    pub address: Option<Address>,
    /// Base repr after parsing before host/plugin levels are added,
    ///
    parsed_node_repr: Repr,
}

/// Type-alias for a task future,
///
type Task = Pin<Box<dyn Future<Output = anyhow::Result<ThunkContext>> + Send + Sync>>;

/// Type-alias for a task fn resource,
///
type TaskFn = Pin<Box<dyn Fn(ThunkContext) -> Task + Send + Sync>>;

impl ActionFactory {
    /// Sets the current address,
    ///
    pub fn set_address(mut self, address: Address) -> Self {
        self.address = Some(address);
        self
    }

    /// Adds a task as a branch from the node level of some plugin P,
    ///
    pub async fn add_task<P>(self, task_name: &str, task: ThunkFn) -> anyhow::Result<Self>
    where
        P: Plugin,
        P::Virtual: NewFn<Inner = P>,
    {
        if let Some(base) = self.address.as_ref() {
            let mut task_repr = self.parsed_node_repr.clone();
            let node = base.node();
            let filter = base.filter_str().expect("should have a filter str");

            let task_addr = format!("{node}?{filter}&task={task_name}");

            let host_level = HostLevel::new(task_addr);
            task_repr
                .upgrade(CrcInterner::default(), host_level)
                .await?;

            let plugin_level = PluginLevel::new_with::<P>(task);
            task_repr
                .upgrade(CrcInterner::default(), plugin_level)
                .await?;

            let plugin_init = self
                .storage
                .storage
                .read()
                .await
                .current_resource::<P>(self.attribute.transmute())
                .expect("should exist since action factory was created");

            let task_key = ResourceKey::<P>::with_repr(task_repr);
            {
                let mut node = self.storage.storage.write().await;
                if let Some(mut parsed_node) = node.resource_mut::<ParsedNode>(ResourceKey::root())
                {
                    parsed_node.attributes.push(task_key.transmute());
                }
                node.put_resource(plugin_init, task_key);
                drop(node);
            }
        }

        Ok(self)
    }

    /// Publish all tasks found in the current parsed node available for publishing,
    ///
    pub async fn publish_all(self, eh: EngineHandle) -> anyhow::Result<Vec<Address>> {
        let mut published = vec![];

        let tc: ThunkContext = self.storage.into();

        if let Some(parsed_node) = tc
            .node()
            .await
            .current_resource::<ParsedNode>(ResourceKey::root())
        {
            for a in parsed_node.attributes {
                if let Some(host) = a.host().and_then(|h| h.address()) {
                    if a.plugin().is_some() {
                        let mut publishing = tc.clone();
                        publishing.set_attribute(a);
                        trace!("Publishing {}", host);

                        publishing.set_property("address", host.as_str())?;

                        let p = eh.publish(publishing).await?;
                        published.push(p);
                    }
                }
            }
        }

        Ok(published)
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

        let tc = self.as_ref().clone();

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
            let result = taskfn(tc).await.map(Some)?;
            {
                let mut node = self.as_ref().node.storage.write().await;
                node.put_resource(taskfn, key.transmute());
            }

            return Ok(result);
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
            
            # -- Testing action publishing
            : .field test_custom_action_field

            + .host test_host
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
    #[reality(attribute_type)]
    field: CustomActionField,
}

#[derive(Reality, Debug, Default, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[reality(call = custom_action_field, plugin, rename = "custom-action-field", group = "test")]
struct CustomActionField {
    #[reality(derive_fromstr)]
    name: String,
}

async fn custom_action_field(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<CustomActionField>().await;
    eprintln!("{:?}", init);
    Ok(())
}

/// Example of bootstrapping resources,
///
async fn custom_action(_tc: &mut ThunkContext) -> anyhow::Result<()> {
    let eh = _tc
        .engine_handle()
        .await
        .expect("should be bound to an engine");

    let host = eh.hosted_resource("engine://test_host").await?;

    // Create the local action
    let action = HostAction::new(host.context().attribute);

    if let Some(recv) = _tc.attribute.recv() {
        if let Some(fields) = recv.fields() {
            let field = fields.first().unwrap();

            let action = action.build(CustomActionField::default(), *field).await?;

            // Add a task
            let action = action
                .add_task::<CustomActionField>("test_123", |tc| {
                    tc.spawn(|mut tc| async move {
                        eprintln!("test_123");
                        tc.take_cache::<usize>();

                        Ok(tc)
                    })
                })
                .await?;

            let action = action
                .add_task::<CustomActionField>("test_432", |tc| {
                    tc.spawn(|tc| async move {
                        eprintln!("test_432");
                        Ok(tc)
                    })
                })
                .await?;

            let published = action.publish_all(eh.clone()).await?;
            for p in published {
                eprintln!("{}", p);
            }

            eh.run("engine://test_host?test.custom-action-field=test_custom_action_field&task=test_432").await?;
        }
    }

    Ok(())
}
