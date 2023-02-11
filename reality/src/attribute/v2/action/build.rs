use std::sync::Arc;

use specs::{EntityBuilder, Entity};

use crate::v2::{Attribute, Error};


/// Trait to build components for an entity,
/// 
pub trait Build
where
    Self: Send + Sync + 'static
{
    /// Builds an entity from an attribute,
    /// 
    fn build(self: Arc<Self>, attribute: &Attribute, entity_builder: EntityBuilder) -> Result<Entity, Error>;
}
