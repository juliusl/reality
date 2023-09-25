use crate::prelude::BoxedNode;

/// When parsing runmd blocks providers are called when a block is loaded, before any nodes are added,
/// 
pub trait Provider {
    /// Returns a node if the parameters are valid,
    /// 
    fn provide(&self, ty: Option<&str>, moniker: Option<&str>) -> Option<BoxedNode>;
}