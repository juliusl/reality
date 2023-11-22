use std::pin::Pin;

use anyhow::anyhow;
use async_trait::async_trait;
use futures_util::Future;
use reality::attributes;
use uuid::Uuid;

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
            decoration: self.context().decoration.clone(),
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

impl Action for ThunkContext {
    fn address(&self) -> String {
        self.property("address")
            .map(|s| s.to_string())
            .unwrap_or(self.variant_id.unwrap_or(Uuid::new_v4()).to_string())
    }

    fn bind(&mut self, context: ThunkContext) {
        *self = context;
    }

    fn bind_node(&mut self, node: ResourceKey<attributes::Node>) {
        self.write_cache(node)
    }

    fn bind_plugin(&mut self, plugin: ResourceKey<attributes::Attribute>) {
        self.attribute = plugin;
    }

    fn node_rk(&self) -> ResourceKey<attributes::Node> {
        self.cached().unwrap_or_default()
    }

    fn plugin_rk(&self) -> ResourceKey<attributes::Attribute> {
        self.attribute
    }

    fn context(&self) -> &ThunkContext {
        self
    }

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

    unsafe {
        let mut node = tc.node_mut().await;
        node.put_resource::<ThunkFn>(
            |tc| {
                CallOutput::Spawn(tc.spawn(|tc| async move {
                    eprintln!("hello world");
                    Ok(tc)
                }))
            },
            rk.transmute(),
        );
    }

    let r = tc.into_hosted_resource();
    assert_eq!(r.address(), uuid.to_string());
    assert_eq!(r.plugin_rk(), rk);
    let _ = r.spawn_call().await.unwrap(); // Will panic if the thunk fn was not called
    ()
}

#[tokio::test]
#[tracing_test::traced_test]
async fn test_custom_action() {
    let builder = define_engine(&[
        |mut eb| {
            eb.enable::<CustomAction>();
        
            eb.workspace_mut().add_buffer(
                "test-custom-action.md",
                r#"
            ```runmd
            + .operation a
            <test.customaction>     test_action
            |# address      =       test://custom-action
            ```
            "#,
            );
            eb
        }
    ]);

    let _engine = builder.compile().await;
    eprintln!("{:#?}", _engine);
    eprintln!("{:#?}", _engine.block());

    let (eh, _) = _engine.spawn(|_, p| Some(p));
    let _tc = eh.run("engine://a").await.unwrap();
    let addr = eh.publish(_tc.transient.into()).await.unwrap();
    eprintln!("{addr}");

    let ca = eh.hosted_resource(addr.to_string()).await.unwrap();
    let _ = ca.spawn_call().await;
    ()
}

#[derive(Reality, Default, Clone)]
#[reality(call = custom_action, plugin, group = "test")]
pub struct CustomAction {
    #[reality(derive_fromstr)]
    name: String,
}

/// Example of bootstrapping resources,
/// 
async fn custom_action(_tc: &mut ThunkContext) -> anyhow::Result<()> {
    eprintln!("custom action init");
    let _ = Local.create::<CustomAction>(_tc).await;

    _tc.store_kv::<ThunkFn>("test 123", |tc| {
        CallOutput::Spawn(tc.spawn(|tc| async move {
            eprintln!("test 123");
            Ok(tc)
        }))
    });

    let mut transient = _tc.transient_mut().await;

    // Get decorations
    if let Some(deco) = _tc.decoration.as_ref() {
        eprintln!("{:#?}", deco);
        transient.put_resource(
            deco.clone(),
            _tc.attribute.transmute()
        );
    }

    // Add a new entrypoint for the resource
    transient.put_resource::<ThunkFn>(
        |tc| {
            CallOutput::Spawn(tc.spawn(|tc| async move {
                assert!(tc.kv_contains::<ThunkFn>("test 123"));

                eprintln!("hello world {:?}", tc.decoration);

                if let Some((_, t)) = tc.fetch_kv::<ThunkFn>("test 123") {
                    if let Ok(Some(tc)) = t(tc.clone()).await {
                        return Ok(tc);
                    }
                }

                Ok(tc)
            }))
        },
        _tc.attribute.transmute(),
    );

    // Clone the cache,
    transient.put_resource(_tc.clone_cache(), _tc.attribute.transmute());

    // Set the default attribute key
    transient.put_resource(_tc.attribute, ResourceKey::root());
    Ok(())
}

async fn __publish_local_action(_tc: &mut ThunkContext) -> anyhow::Result<()> {
    Ok(())
}