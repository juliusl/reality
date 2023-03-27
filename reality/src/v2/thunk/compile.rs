use std::sync::Arc;
use async_trait::async_trait;
use crate::v2::Properties;
use crate::Error;
use crate::v2::compiler::BuildRef;

/// Trait to compile components for an entity,
/// 
#[async_trait]
pub trait Compile
where
    Self: Send + Sync
{
    /// Compiles a built object,
    /// 
    async fn compile<'a, 'b>(&'a self, build_ref: BuildRef<'b, Properties>) -> Result<(), Error>;
}

/// Implementation to use as a Thunk component,
/// 
#[async_trait]
impl Compile for Arc<dyn Compile> {
    async fn compile<'a, 'b>(&'a self, build_ref: BuildRef<'b, Properties>) -> Result<(), Error> {
        self.as_ref().compile(build_ref).await
    }
}
