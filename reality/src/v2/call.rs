use std::{sync::Arc, ops::Deref};

use async_trait::async_trait;

use crate::Error;

use super::Properties;

/// Trait for a type to implement an async call function,
/// 
#[async_trait]
pub trait Call
    where
        Self: Send + Sync
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
