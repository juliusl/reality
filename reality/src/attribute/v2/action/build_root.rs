use std::sync::Arc;

use specs::{EntityBuilder, Entity};

use crate::v2::{Root, Error};

/// Trait for building an entity from a root,
///
pub trait BuildRoot
where
    Self: Send + Sync + 'static
{
    /// Builds an entity from a root,
    /// 
    fn build_root<'a>(self: Arc<Self>, root: &'a Root<'a>, entity_builder: EntityBuilder) -> Result<Entity, Error>;
}

