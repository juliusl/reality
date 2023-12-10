use tokio::task::JoinHandle;

use super::prelude::ThunkContext;

/// Trait for implementing call w/ an async trait,
///
/// **Note** This is a convenience if the additional Skip/Abort control-flow options
/// are not needed.
///
#[async_trait::async_trait]
pub trait CallAsync {
    /// Executed by `ThunkContext::spawn`,
    ///
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()>;
}

/// Type alias for the result of spawning a task,
///
pub type SpawnResult = Option<JoinHandle<anyhow::Result<ThunkContext>>>;

/// Enumeration of output a plugin can return,
///
pub enum CallOutput {
    /// The plugin has spawned a task,
    ///
    /// If a join-handle was successfully created, then it will be polled to completion and the result will be passed to the next plugin.
    ///
    Spawn(SpawnResult),
    /// The context has an update,
    ///
    Update(Option<ThunkContext>),
    /// The plugin has decided to abort further execution,
    ///
    Abort(anyhow::Result<()>),
    /// This call should be skipped,
    ///
    Skip,
}

impl From<SpawnResult> for CallOutput {
    fn from(value: SpawnResult) -> Self {
        CallOutput::Spawn(value)
    }
}

impl From<anyhow::Result<()>> for CallOutput {
    fn from(value: anyhow::Result<()>) -> Self {
        CallOutput::Abort(value)
    }
}
