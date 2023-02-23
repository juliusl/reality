use std::ops::Deref;
use std::sync::Arc;

use specs::world::LazyBuilder;
use specs::Entity;

use crate::Error;

/// Trait to build components for an entity,
/// 
pub trait Build
where
    Self: Send + Sync
{
    /// Builds an entity w/ a lazy builder
    /// 
    fn build(&self, lazy_builder: LazyBuilder) -> Result<Entity, Error>;
}

impl Build for Arc<dyn Build> {
    fn build(&self, lazy_builder: LazyBuilder) -> Result<Entity, Error> {
        self.deref().build(lazy_builder)
    }
}