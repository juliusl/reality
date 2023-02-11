use std::sync::Arc;

use crate::attribute::v2::Error;

/// Trait for building an entity from a toml document,
/// 
pub trait EditToml
where
    Self: Send + Sync + 'static
{
    /// Edits and returns an updated toml document,
    /// 
    fn edit_toml(self: Arc<Self>, previous: &toml_edit::Document) -> Result<toml_edit::Document, Error>;
}

