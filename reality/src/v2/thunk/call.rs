use async_trait::async_trait;
use futures::Future;
use std::ops::Deref;
use std::sync::Arc;

use crate::Result;

use super::{AsyncDispatch, Properties};

use crate::v2::prelude::*;

internal_use!();

/// Trait for a type to implement an async call function,
///
#[thunk]
#[async_trait]
pub trait Call
{
    /// Returns properties map,
    ///
    async fn call(&self) -> Result<Properties>;
}


/// This implementation is for functions w/o any state,
///
#[async_trait]
impl<F, T> Call for T
where
    F: Future<Output = Result<Properties>> + Send + Sync + 'static,
    T: Fn() -> F + Send + Sync + 'static,
{
    async fn call(&self) -> Result<Properties> {
        self().await
    }
}

#[async_trait]
impl<C: Call + Send + Sync> AsyncDispatch for Arc<C> {
    async fn async_dispatch<'a, 'b>(
        &'a self,
        build_ref: DispatchRef<'b, Properties>,
    ) -> DispatchResult<'b> {
        build_ref
            .enable_async()
            .transmute::<ThunkCall>()
            .map_into::<Properties, _>(|r| {
                let r = r.clone();
                async move { r.call().await }
            })
            .await
            .disable_async()
            .result()
    }
}
