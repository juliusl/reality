use crate::AttributeParser;

/// The root trait represents a custom stable attribute acting as a root container for state extensions,
/// 
pub trait Root {
    /// Identifier for this root,
    /// 
    fn ident() -> String;

    /// Returns an attribute parser given an extension symbol and input,
    /// 
    fn compile_extension(self, extension_name: impl Into<String>, input: impl Into<String>) -> Option<AttributeParser>;
}

