use crate::{BlockProperties, Identifier};

use super::{Block, Attribute};

/// Visitor trait for visiting compiled runmd data,
/// 
pub trait Visitor {
    /// Visits a block,
    /// 
    fn visit_block(&mut self, block: &Block);

    /// Visits an attribute root,
    /// 
    fn visit_root(&mut self, root: &Attribute);

    /// Visits a root extension,
    /// 
    fn visit_extension(&mut self, identifier: &Identifier, properties: &BlockProperties);
}