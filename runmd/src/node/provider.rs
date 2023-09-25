use super::BoxedNode;

/// When parsing runmd blocks providers are called when a node should be added,
/// 
pub trait Provider {
    /// Returns a node if the parameters are valid,
    /// 
    fn provide(&self, name: &str, tag: Option<&str>, input: Option<&str>) -> Option<BoxedNode>;
}