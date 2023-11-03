use crate::prelude::BoxedNode;

use super::Info as BlockInfo;

/// When parsing runmd blocks providers are called when a block is loaded, before any nodes are added,
/// 
pub trait Provider {
    /// Returns a node if the parameters are valid,
    /// 
    fn provide(&self, block_info: BlockInfo) -> Option<BoxedNode>;
}