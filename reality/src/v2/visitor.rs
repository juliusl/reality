use crate::{BlockProperties, Identifier};

use super::{Block, Root};

/// Visitor trait for visiting compiled runmd data,
/// 
pub trait Visitor {
    /// Visits a block,
    /// 
    fn visit_block(&mut self, block: &Block);

    /// Visits a root,
    /// 
    fn visit_root(&mut self, root: &Root);

    /// Visits a root extension,
    /// 
    fn visit_extension(&mut self, identifier: &Identifier, properties: &BlockProperties);
}