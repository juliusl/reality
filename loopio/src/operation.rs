use std::fmt::Debug;

use futures_util::Future;
use futures_util::FutureExt;

use anyhow::anyhow;

use futures_util::StreamExt;
use futures_util::TryStreamExt;
use reality::Attribute;
use reality::ResourceKey;
use reality::SetIdentifiers;
use reality::StorageTarget;
use reality::ThunkContext;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::trace;
use tracing::warn;

use crate::prelude::Action;
use crate::prelude::Instruction;
use reality::prelude::*;

/// Struct for a top-level node,
///
#[derive(Reality, Default)]
#[reality(call = debug_op, plugin)]
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

async fn debug_op(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let mut init = tc.initialized::<Operation>().await;
    init.bind(tc.clone());
    // eprintln!("Found operation {:?}", init);
    // let block = tc.parsed_block().await.unwrap_or_default();
    // if let Some(node) = block.nodes.get(&init.node) {
    //     // eprintln!("Found node.....{:?}", init.node.transmute::<Attribute>());
    //     // eprintln!("{:#?}", node.doc_headers.get(&init.node.transmute()));
    //     // eprintln!("{:#?}", node.comment_properties.get(&init.node.transmute()));
    // }

    let node = tc.node().await;
    let _tc = node
        .stream_attributes()
        .map(Ok)
        .try_fold(tc.clone(), |tc, attr| async move {
            // eprintln!("-- should call -- {:?}", attr);
            // TODO -- Can add the plumbing for the activity system through here
            // 
            {
                let node = tc.node().await;
                let mut tc = tc.clone();
                #[allow(unused_assignments)]
                let mut previous = tc.clone();
                if let Some(func) = node.current_resource::<ThunkFn>(Some(attr.transmute())) {
                    previous = tc.clone();
                    tc.attribute = Some(attr);
                    match (func)(tc.clone()) {
                        CallOutput::Spawn(Some(jh)) => {
                            tc = jh.await??;
                        }
                        CallOutput::Skip | CallOutput::Spawn(None) => {
                            tc = previous;
                        }
                        CallOutput::Abort(err) => err?,
                    }
                } else {
                    // TODO -- The expectation is that each of these attributes has a thunk fn
                    // For now make this very loud to fix any edge cases
                    eprintln!("ERROR!! ======== NO THUNK FN {:?}", attr);
                }

                Ok::<ThunkContext, anyhow::Error>(tc)
            }
        })
        .await?;

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

    /// Executes the operation,
    ///
    pub async fn execute(&self) -> anyhow::Result<ThunkContext> {
        if let Some(context) = self.context.clone() {
            context
                .apply_thunks_with(|c, _next| async move {
                    trace!("Executing next {:?}", _next);
                    Ok(c)
                })
                .await
        } else {
            Err(anyhow!("Could not execute operation, "))
        }
    }

    /// Spawns the underlying operation, storing a handle anc cancellation token in the current struct,
    ///
    pub fn spawn(&mut self) {
        if self.spawned.is_some() {
            warn!("Existing spawned task exists");
        }

        if let Some(cancelled) = self.context.as_ref().map(|c| c.cancellation.clone()) {
            let spawned = self.clone();
            self.spawned = Some((
                cancelled,
                tokio::spawn(async move { spawned.execute().await }),
            ));
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

    /// Navigates a path to a thunk context,
    ///
    pub async fn navigate(&self, path: impl AsRef<str>) -> anyhow::Result<ThunkContext> {
        if let Some(tc) = self.context.as_ref() {
            if let Some(tc) = tc.navigate(path.as_ref()).await {
                let tc = tc.context().call().await?;
                if let Some(tc) = tc {
                    return Ok(tc);
                }
            }
        }

        Err(anyhow!("Could not find path: {}", path.as_ref()))
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
                return std::task::Poll::Ready(Err(anyhow::anyhow!("Operation has been cancelled")));
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
    fn address(&self) -> String {
        if let Some(tag) = self.tag.as_ref() {
            format!("{}#{}", self.name, tag)
        } else {
            self.name.to_string()
        }
    }

    fn bind(&mut self, context: ThunkContext) {
        self.context = Some(context);
    }

    fn context(&self) -> &ThunkContext {
        self.context.as_ref().expect("should be bound to an engine")
    }

    fn context_mut(&mut self) -> &mut ThunkContext {
        self.context.as_mut().expect("should be bound to an engine")
    }

    fn bind_node(&mut self, node: ResourceKey<reality::attributes::Node>) {
        self.node = node;
    }

    fn node_rk(&self) -> ResourceKey<reality::attributes::Node> {
        self.node
    }

    fn bind_plugin(&mut self, _: ResourceKey<reality::attributes::Attribute>) {}

    fn plugin_rk(&self) -> Option<ResourceKey<reality::attributes::Attribute>> {
        None
    }
}

impl SetIdentifiers for Operation {
    fn set_identifiers(&mut self, name: &str, tag: Option<&String>) {
        self.name = name.to_string();
        self.tag = tag.cloned();
    }
}

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
    <loopio.std.io.println>                     Hello World a
    |# listen = event_test

    # -- Example plugin b
    <loopio.std.io.println>                     Hello World b
    |# listen = event_test_b

    # -- Another example
    + .operation b
    |# test = test

    # -- Example plugin
    # -- Example plugin in operation b
    <a/loopio.std.io.println>                   Hello World a
    |# listen = event_test

    # -- Example plugin b
    <loopio.std.io.println>                     Hello World b
    |# listen = event_test_b

    # -- # Example demo host
    + .host demo

    # -- # Example of a mapped action to an operation
    : .action   b
    |# help = example of mapping to an operation

    # -- # Example of a mapped action to within an operation
    : .action   b/a/loopio.std.io.println
    |# help = example of mapping to an operation within an operation

    ```
    "#,
    );

    let engine = crate::engine::Engine::builder().build();
    let engine = engine.compile(workspace).await;

    // let block = engine.block.clone().unwrap_or_default();
    // for (n, node_storage) in engine.nodes.iter() {
    //     if let Some(node) = block.nodes.get(n) {
    //         for attr in node.attributes.iter() {
    //             trace!("{:?}", attr);
    //             let mut context = engine.new_context(node_storage.clone()).await;
    //             context.attribute = Some(*attr);

    //             let _ = context.spawn_call().await.unwrap().unwrap();
    //         }
    //     }
    // }

    // for (_, op) in engine.iter_operations() {
    //     if !op.context().node().await.contains::<ThunkFn>(None) {
    //         panic!();
    //     }
    //     op.context().call().await.unwrap().unwrap();
    // }

    // for (_, seq) in engine.iter_sequences() {
    //     let seq = seq.into_hosted_resource();
    //     eprintln!("Seq -- {:#?}", seq);
    // }

    // for (_, host) in engine.iter_hosts() {
    //     let hosted = host.into_hosted_resource();
    //     eprintln!("Host -- {:#?}", hosted);
    // }

    // let block = engine.block().unwrap();

    // let deck = Deck::from(block);
    // eprintln!("{:#?}", deck);

    // eprintln!("{:#?}", block);
    // for (_, n) in block.nodes.iter() {
    //     for (_rk, v) in n.comment_properties.iter() {
    //         for (k, v) in v {
    //             if k == "listen" || k == "notify" {

    //             }
    //         }
    //     }
    // }

    // let host = engine.get_host("demo").await;
    // let _e = engine.spawn(|_, p| Some(p));

    // if let Some(mut host) = host {
    //     let action = host.spawn().expect("should be able to get an action");
    //     action
    //         .await
    //         .unwrap()
    //         .expect("should be able to await the action");
    // }

    // let op = engine.get_operation("a").await.unwrap();
    // op.await.unwrap();

    ()
}
