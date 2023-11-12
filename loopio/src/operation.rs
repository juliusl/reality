use std::fmt::Debug;

use futures_util::Future;
use futures_util::FutureExt;

use anyhow::anyhow;

use reality::ThunkContext;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Struct for a top-level node,
///
pub struct Operation {
    /// Name of this operation,
    ///
    name: String,
    /// Tag allowing operation variants
    ///
    tag: Option<String>,
    /// Thunk context of the operation,
    ///
    context: Option<ThunkContext>,
    /// Running operation,
    ///
    spawned: Option<(CancellationToken, JoinHandle<anyhow::Result<ThunkContext>>)>,
}

pub struct Spawned {
    pub started: tokio::time::Instant,
    pub cancel: CancellationToken,
    pub task: JoinHandle<anyhow::Result<ThunkContext>>,
}

impl Clone for Operation {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            tag: self.tag.clone(),
            context: self.context.clone(),
            spawned: None,
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
        }
    }

    /// Returns the address to use w/ this operation,
    ///
    pub fn address(&self) -> String {
        if let Some(tag) = self.tag.as_ref() {
            format!("{}#{}", self.name, tag)
        } else {
            self.name.to_string()
        }
    }

    /// Binds operation to a thunk context,
    ///
    pub fn bind(&mut self, context: ThunkContext) {
        self.context = Some(context);
    }

    /// Returns a reference to the inner context,
    ///
    pub fn context(&self) -> Option<&ThunkContext> {
        self.context.as_ref()
    }

    /// Returns a mutable reference to the inner context,
    ///
    pub fn context_mut(&mut self) -> Option<&mut ThunkContext> {
        self.context.as_mut()
    }

    /// Executes the operation,
    ///
    pub async fn execute(&self) -> anyhow::Result<ThunkContext> {
        if let Some(context) = self.context.clone() {
            context.apply_thunks_with(|c, _next| async move {
                eprintln!("Executing next {:?}", _next);
                Ok(c)
            }).await
        } else {
            Err(anyhow!("Could not execute operation, "))
        }
    }

    /// Executes plugins that match the filter,
    /// 
    pub async fn filter_execute(&self, filter: impl Into<String>) -> anyhow::Result<ThunkContext> {
        if let Some(context) = self.context.as_ref().map(|c| c.filter(filter)) {
            context.apply_thunks().await
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
                let tc = tc.call().await?;
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

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        if let Some((cancelled, mut spawned)) = self.as_mut().spawned.take() {
            if cancelled.is_cancelled() {
                return std::task::Poll::Ready(Err(anyhow::anyhow!("Operation has been cancelled")))
            }

            match spawned.poll_unpin(cx) {
                std::task::Poll::Ready(Ok(result)) => std::task::Poll::Ready(result),
                std::task::Poll::Pending => {
                    self.spawned = Some((cancelled, spawned));
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                },
                std::task::Poll::Ready(Err(err)) => {
                    std::task::Poll::Ready(Err(err.into()))
                }
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
