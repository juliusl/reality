mod provider;
use async_trait::async_trait;
pub use provider::Provider;

mod info;
pub use info::Info as NodeInfo;

use crate::prelude::BlockInfo;

/// Type-alias for a boxed node type,
///
pub type BoxedNode = std::pin::Pin<Box<dyn Node + Unpin + Send + Sync>>;

/// Trait for types that consume instructions from a runmd block,
///
#[async_trait(?Send)]
pub trait Node: crate::prelude::ExtensionLoader + std::fmt::Debug {
    /// Assigns a path to this node,
    ///
    fn assign_path(&mut self, path: String);

    /// Sets the block info for this node,
    ///
    /// Block info details the location within the block this node belongs,
    ///
    fn set_info(&mut self, node_info: NodeInfo, block_info: BlockInfo);

    /// Called after a line is parsed,
    ///
    fn parsed_line(&mut self, _node_info: NodeInfo, _block_info: BlockInfo) {}

    /// Called when the **entire** block this node belongs to has completed parsing,
    ///
    /// **Note**: At this final step, the node will be un-pinned and references to it will be dropped
    /// from parser state.
    ///
    fn completed(self: Box<Self>);

    /// Define a property for this node,
    ///
    async fn define_property(&mut self, name: &str, tag: Option<&str>, input: Option<&str>);
}
