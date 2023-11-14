use super::prelude::*;

/// Contains line data derived by the lexer
/// 
#[derive(Hash, Default, Debug, Clone)]
pub struct Line<'a> {
    /// Instruction for this line,
    /// 
    pub instruction: Instruction,
    /// Extension value if provided,
    /// 
    pub extension: Option<Extension<'a>>,
    /// Tag value if provided,
    /// 
    pub tag: Option<Tag<'a>>,
    /// Attribute value,
    /// 
    pub attr: Option<Attribute<'a>>,
    /// Comment value,
    /// 
    pub comment: Option<Vec<&'a str>>,
}