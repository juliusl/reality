use specs::{EntityBuilder, Entity};

use crate::BlockProperties;

use super::CompileError;

/// The extension trait supplies the compile-time implementation of for the `<>` extension keyword,
/// 
pub trait Extension {
    /// Identifier for this extension,
    /// 
    fn ident() -> String;

    /// Called after all attributes in the extension scope have been parsed,
    /// 
    fn compile(entity_builder: EntityBuilder, properties: BlockProperties) -> Result<Entity, CompileError>;
}
