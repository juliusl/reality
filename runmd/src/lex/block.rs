use super::prelude::*;

/// A block containing the analysis results,
/// 
#[derive(Default, Debug, Clone)]
pub struct Block<'a> {
    /// Media type of this block,
    /// 
    pub ty: Option<&'a str>,
    /// Moniker for this block,
    /// 
    pub moniker: Option<&'a str>,
    /// Lines analyzed by the lexer,
    /// 
    pub lines: Vec<Line<'a>>,
}
