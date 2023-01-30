use specs::{EntityBuilder, Entity};

use crate::BlockProperties;

use super::{CompileError, ExtensionParser};

/// The extension trait supplies the compile-time implementation of for the `<>` extension keyword,
/// 
/// # Background
/// In the case of using the compiler to compile runmd, the extension allows for an initial pass in order to collect block properties,
/// and a finishing pass in order to compose Components based on those properties.
/// 
/// When using an extension it is also possible to specify the target world to build the resulting entity.
/// 
pub trait Extension<Parser>
where
    Parser: ExtensionParser
{
    /// Identifier for this extension,
    /// 
    fn ident() -> &'static str;

    /// Can declare custom attribute types that map to configurable properties of this extension,
    /// 
    fn parser(extension_parser: &mut Parser);

    /// Called after all properties in the extension scope have been parsed,
    /// 
    fn compile(entity_builder: EntityBuilder, properties: BlockProperties) -> Result<Entity, CompileError>;

    /// Returns the extension implemenation as a thunk struct,
    /// 
    fn as_thunk() -> ExtensionThunk<Parser> {
        ExtensionThunk(Self::parser, Self::compile)
    }
}

/// Type alias for the parser function of the Extension trait,
/// 
pub type ExtensionParserFunc<T> = fn(&mut T);

/// Type alias for the compile function of the Extension trait,
/// 
pub type ExtensionCompileFunc = fn(EntityBuilder, BlockProperties) -> Result<Entity, CompileError>;

/// Struct for concrete function pointers of an Extension implemenation,
/// 
pub struct ExtensionThunk<Parser: ExtensionParser>(pub ExtensionParserFunc<Parser>, pub ExtensionCompileFunc);
