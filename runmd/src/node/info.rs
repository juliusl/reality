use crate::lex::prelude::Line;

/// Struct containing metadata of the node while it is being parsed,
///
#[derive(Hash, Debug, Clone)]
pub struct Info<'a> {
    /// Index of this node,
    ///
    pub idx: usize,
    /// Index of the parent of this node,
    ///
    pub parent_idx: Option<usize>,
    /// Line this node was parsed from,
    ///
    pub line: Line<'a>,
    /// Location from the source input where this node was analyzed from,
    ///
    pub span: Option<logos::Span>,
}

impl Info<'_> {
    /// Returns the comment from the line if one is set,
    ///
    pub fn get_comment(&self) -> Option<String> {
        self.line.comment.clone().map(|s| s.join("\n"))
    }
}
