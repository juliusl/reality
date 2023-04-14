use crate::v2::prelude::*;

internal_use!();

/// Trait to build components for an entity,
///

#[thunk]
pub trait Build
{
    /// Builds an entity w/ a lazy builder
    ///
    fn build(&self, lazy_builder: LazyBuilder) -> Result<Entity>;
}


impl<T: Fn(LazyBuilder) -> Result<Entity> + Sync + Send + 'static> Build for T {
    fn build(&self, lazy_builder: LazyBuilder) -> Result<Entity> {
        self(lazy_builder)
    }
}