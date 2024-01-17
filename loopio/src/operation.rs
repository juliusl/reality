use bytes::Bytes;
use futures_util::Future;
use futures_util::FutureExt;
use std::fmt::Debug;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::info;

use crate::prelude::*;

/// Struct for a top-level node,
///
#[derive(Reality, Default)]
#[plugin_def(call = run_operation)]
pub struct Operation {
    /// Name of this operation,
    ///
    #[reality(derive_fromstr)]
    name: String,
    /// Tag allowing operation variants
    ///
    #[reality(ignore)]
    tag: Option<String>,
    /// Thunk context of the operation,
    ///
    #[reality(ignore)]
    context: Option<ThunkContext>,
    /// Running operation,
    ///
    #[reality(ignore)]
    spawned: Option<(CancellationToken, JoinHandle<anyhow::Result<ThunkContext>>)>,
    /// Node attribute,
    ///
    #[reality(ignore)]
    node: ResourceKey<reality::attributes::Node>,
}

/// **Main** entrypoint for operations defined w/ .runmd. Will call plugins in the order their
/// extensions were defined under the operation node.
///
async fn run_operation(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let mut init = tc.initialized::<Operation>().await;
    init.bind(tc.clone());

    info!(op = init.name, "Starting operation");

    if let Some(eh) = tc.engine_handle().await {
        if let Some(host) = eh.host {
            debug!(op = init.name, "Host is set on operation -- {:?}", host);
        }
    }

    if let Some(host) = tc.attribute.host() {
        let ext = host.extensions();
        if let Some(ext) = ext {
            let mut context = tc.clone();
            for e in ext.iter() {
                info!(op = init.name, "Running next operation step -- {}", e);

                let attr = ResourceKey::<Attribute>::with_repr(*e);
                context.set_attribute(attr);

                // If set, listens for an event before continuing to call the next ext
                if let Some(message) = context.listen().await? {
                    #[cfg(feature = "flexbuffers-ext")]
                    use crate::prelude::flexbuffers_ext::FlexbufferCacheExt;

                    #[cfg(feature = "flexbuffers-ext")]
                    context.set_flexbuffer_root(message.clone());

                    #[cfg(not(feature = "flexbuffers-ext"))]
                    context.store_kv("inbound_event_message", message);
                }

                context = context.call().await?.unwrap_or(context);

                context.process_node_updates().await;

                // TODO: If context contains a LocalAction/RemoteAction, auto publish the transient

                // **Note** --
                // If the plugin being called is long-running,
                // this will need to be called from within the plugin's call fn.
                //
                // If set, notifies an event before continuing to the call the next ext
                //
                context
                    .notify(
                        context
                            .fetch_kv::<Bytes>("outbound_event_message")
                            .map(|b| b.1.clone()),
                    )
                    .await?;
            }
            tc.transient = context.transient.clone();
            debug!(
                "Before returning transient is -- {}",
                tc.transient.initialized()
            );
        }
    }

    Ok(())
}

impl Clone for Operation {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            tag: self.tag.clone(),
            context: self.context.clone(),
            spawned: None,
            node: self.node,
        }
    }
}

impl Operation {
    /// Creates a new operation,
    ///
    pub fn new(name: impl Into<String>, tag: Option<String>) -> Self {
        Self {
            name: name.into(),
            tag,
            context: None,
            spawned: None,
            node: Default::default(),
        }
    }

    /// Returns true if the underlying spawned operation has completed,
    ///
    pub fn is_finished(&self) -> bool {
        self.spawned
            .as_ref()
            .map(|(_, j)| j.is_finished())
            .unwrap_or_default()
    }

    /// Returns true if the underlying operation is active,
    ///
    pub fn is_running(&self) -> bool {
        self.spawned.is_some()
    }

    /// Waits for the underlying spawned task to complete,
    ///
    pub async fn wait_result(&mut self) -> anyhow::Result<ThunkContext> {
        if let Some((_, task)) = self.spawned.take() {
            task.await?
        } else {
            Err(anyhow::anyhow!("Task is not spawned"))
        }
    }

    /// Blocks until the task returns a result,
    ///
    pub fn block_result(&mut self) -> anyhow::Result<ThunkContext> {
        if let Some((_, task)) = self.spawned.take() {
            futures::executor::block_on(task)?
        } else {
            Err(anyhow::anyhow!("Task is not spawned"))
        }
    }

    /// Cancels the running task,
    ///
    pub async fn cancel(&mut self) -> anyhow::Result<ThunkContext> {
        if let Some((cancel, task)) = self.spawned.take() {
            cancel.cancel();
            task.await?
        } else {
            Err(anyhow::anyhow!("Task is not spawned"))
        }
    }
}

impl Future for Operation {
    type Output = anyhow::Result<ThunkContext>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Some((cancelled, mut spawned)) = self.as_mut().spawned.take() {
            if cancelled.is_cancelled() {
                return std::task::Poll::Ready(Err(anyhow::anyhow!(
                    "Operation has been cancelled"
                )));
            }

            match spawned.poll_unpin(cx) {
                std::task::Poll::Ready(Ok(result)) => std::task::Poll::Ready(result),
                std::task::Poll::Pending => {
                    self.spawned = Some((cancelled, spawned));
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
                std::task::Poll::Ready(Err(err)) => std::task::Poll::Ready(Err(err.into())),
            }
        } else {
            self.spawn();
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    }
}

impl Debug for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Operation")
            .field("name", &self.name)
            .field("tag", &self.tag)
            .finish()
    }
}

impl Action for Operation {
    #[inline]
    fn address(&self) -> String {
        if let Some(tag) = self.tag.as_ref() {
            format!("{}#{}", self.name, tag)
        } else {
            self.name.to_string()
        }
    }

    #[inline]
    fn bind(&mut self, context: ThunkContext) {
        self.context = Some(context);
    }

    #[inline]
    fn context(&self) -> &ThunkContext {
        self.context.as_ref().expect("should be bound to an engine")
    }

    #[inline]
    fn context_mut(&mut self) -> &mut ThunkContext {
        self.context.as_mut().expect("should be bound to an engine")
    }

    #[inline]
    fn bind_node(&mut self, node: ResourceKey<reality::attributes::Node>) {
        self.node = node;
    }

    #[inline]
    fn node_rk(&self) -> ResourceKey<reality::attributes::Node> {
        self.node
    }

    #[inline]
    fn bind_plugin(&mut self, _: ResourceKey<reality::attributes::Attribute>) {}

    #[inline]
    fn plugin_rk(&self) -> ResourceKey<reality::attributes::Attribute> {
        ResourceKey::root()
    }
}

impl SetIdentifiers for Operation {
    fn set_identifiers(&mut self, name: &str, tag: Option<&String>) {
        self.name = name.to_string();
        self.tag = tag.cloned();
    }
}

#[allow(unused)]
mod test {
    use crate::prelude::*;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_operation() {
        let mut workspace = Workspace::new();
        workspace.add_buffer(
            "demo.md",
            r#"
        ```runmd
        # -- Example operation that listens for the completion of another
        + .operation a
        |# test = test
    
        # -- Example plugin
        # -- Example plugin in operation a
        <builtin.println>                     Hello World a
    
        # -- Example plugin b
        <builtin.println>                     Hello World b
    
        # -- Another example
        + .operation b
        |# test = test
    
        # -- Example plugin
        # -- Example plugin in operation b
        <a/builtin.println>                   Hello World aa
    
        # -- Example plugin b
        <builtin.println>                     Hello World bb
    
        # -- Example demo host
        + .host demo
    
        # -- Example of a mapped action to an operation
        : .action   b
        |# help = example of mapping to an operation
    
        # -- Example of a mapped action to within an operation
        : .action   b/a/builtin.println
        |# help = example of mapping to an operation within an operation
    
        ```
        "#,
        );

        let engine = crate::engine::Engine::builder().build();
        let _engine = engine.compile(workspace).await.unwrap();

        if let Some(package) = _engine.package.as_ref() {
            let mut matches = package.search("println?b=0&n=5");
            let _ = matches
                .pop()
                .unwrap()
                .program
                .context()
                .unwrap()
                .call()
                .await
                .unwrap();

            let mut matches = package.search("a");
            let tc = matches
                .pop()
                .unwrap()
                .program
                .context()
                .unwrap()
                .call()
                .await
                .unwrap();

            let tc = tc.unwrap();

            let packet = tc.attribute.empty_packet();
            eprintln!("{:#?}", packet);
        }

        eprintln!("{:#?}", _engine);

        let mut resource = _engine.get_resource("engine://demo").await.unwrap();

        {
            let node = resource.context().node().await;
            let parsed_node = node.root_ref().current::<ParsedNode>().unwrap();
            eprintln!("{:#?}", parsed_node);
        }

        let host = resource.context_mut().as_remote_plugin::<Host>().await;
        eprintln!("{:#?}", host);

        host.action.iter().for_each(|a| {
            let help = a.property("help");
            eprintln!("{:?}", help);

            let prop = a.property.unwrap();
            if let Some(node) = prop.node() {
                if let Some(annotations) = node.annotations() {
                    eprintln!("{:?}", annotations);
                }
            }
        });

        let resource = _engine.get_resource("demo://b").await.unwrap();
        let _ = resource.spawn().await.unwrap();

        // if let Some(repr) = resource.context().attribute.repr() {
        //     eprintln!("{:#}", repr);
        //     eprintln!("is_link: {}", resource.context().attribute.is_link());
        //     eprintln!("key: {:x}", resource.context().attribute.transmute::<Host>().key());
        // }

        ()
    }
}
