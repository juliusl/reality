
use super::{BoxedNode, BlockInfo, NodeInfo};

/// When parsing runmd blocks providers are called when a node should be added,
/// 
pub trait Provider {
    /// Returns a node if the parameters are valid,
    /// 
    fn provide(&self, name: &str, tag: Option<&str>, input: Option<&str>, node_info: &NodeInfo, block_info: &BlockInfo) -> Option<BoxedNode>;
}