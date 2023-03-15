use std::sync::Arc;
use std::ops::Deref;
use async_trait::async_trait;
use futures::Future;

use crate::Error;

use super::Properties;

/// Trait for a type to implement an async call function,
///
#[async_trait]
pub trait Call
where
    Self: Send + Sync,
{
    /// Returns properties map,
    ///
    async fn call(&self) -> Result<Properties, Error>;
}

#[async_trait]
impl Call for Arc<dyn Call> {
    async fn call(&self) -> Result<Properties, Error> {
        self.deref().call().await
    }
}

/// This implementation is for functions w/o any state,
///
#[async_trait]
impl<
        F: Future<Output = Result<Properties, Error>> + Send + Sync + 'static,
        T: Fn() -> F + Send + Sync + 'static,
    > Call for T
{
    async fn call(&self) -> Result<Properties, Error> {
        self().await
    }
}
