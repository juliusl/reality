use std::sync::Arc;

use specs::{EntityBuilder, Entity, world::LazyBuilder};

use crate::v2::{Attribute, Error};


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
