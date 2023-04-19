use async_trait::async_trait;

use crate::v2::prelude::*;

internal_use!();

/// Trait for a type to implement an async call function,
///
#[thunk]
#[async_trait]
pub trait Call {
    /// Returns properties map,
    ///
    async fn call(&self) -> Result<Properties>;
}
