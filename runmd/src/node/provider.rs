use async_trait::async_trait;

use super::{BlockInfo, BoxedNode, NodeInfo};

/// When parsing runmd blocks providers are called when a node should be added,
///
#[async_trait(?Send)]
pub trait Provider {
    /// Returns a node if the parameters are valid,
    ///
    async fn provide(
        &self,
        name: &str,
        tag: Option<&str>,
        input: Option<&str>,
        node_info: &NodeInfo,
        block_info: &BlockInfo,
    ) -> Option<BoxedNode>;
}
