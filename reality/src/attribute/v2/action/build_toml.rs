use std::sync::Arc;

use specs::{EntityBuilder, Entity};

use crate::attribute::v2::Error;

/// Trait for building an entity from a toml document,
/// 
pub trait BuildToml
where
    Self: Send + Sync + 'static
{
    /// Builds an entity from a toml document,
    /// 
    fn build_toml(self: Arc<Self>, toml: &toml_edit::Document, entity_builder: EntityBuilder) -> Result<Entity, Error>;
}
