use crate::v2::compiler::DispatchRef;
use crate::v2::Properties;
use crate::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Trait to run async code and then dispatch actions to a world,
///
#[async_trait]
pub trait AsyncDispatch<const SLOT: usize = 0>
where
    Self: Send + Sync,
{
    /// Async dispatch fn,
    ///
    async fn async_dispatch<'a, 'b>(
        &'a self,
        build_ref: DispatchRef<'b, Properties>,
    ) -> DispatchResult<'b>;
}

/// Implementation to use as a Thunk component,
///
#[async_trait]
impl<const SLOT: usize> AsyncDispatch for Arc<dyn AsyncDispatch<SLOT>> {
    async fn async_dispatch<'a, 'b>(
        &'a self,
        build_ref: DispatchRef<'b, Properties>,
    ) -> DispatchResult<'b> {
        self.as_ref().async_dispatch(build_ref).await
    }
}

/// Type alias for a dispatch result,
///
pub type DispatchResult<'a> = Result<DispatchRef<'a, Properties>>;

/// Trait to dispatch changes to a world,
///
pub trait Dispatch<const SLOT: usize = 0>
where
    Self: Send + Sync,
{
    /// Compiles the build ref and returns a result containing the build ref,
    ///
    fn dispatch<'a>(&self, dispatch_ref: DispatchRef<'a, Properties>) -> DispatchResult<'a>;
}

