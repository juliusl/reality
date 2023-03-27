use std::sync::Arc;
use async_trait::async_trait;
use crate::v2::Properties;
use crate::Error;
use crate::v2::compiler::BuildRef;

/// Trait to compile components for an entity,
/// 
#[async_trait]
pub trait Compile<T = ()>
where
    Self: Send + Sync
{
    /// Compiles a built object,
    /// 
    async fn compile<'a>(&self, build_ref: BuildRef<'a, Properties>) -> Result<(), Error>;
}

#[async_trait]
impl Compile for Arc<dyn Compile> {
    async fn compile<'a>(&self, build_ref: BuildRef<'a, Properties>) -> Result<(), Error> {
        self.as_ref().compile(build_ref).await
    }
}
