use anyhow::anyhow;

use crate::ParsedNode;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;
use crate::ThunkContext;

/// Wrapper struct over a parsed node and it's corresponding storage,
///
pub struct Program {
    /// Parsed node data,
    ///
    node: ParsedNode,
    /// Shared storage,
    ///
    storage: Shared,
}

impl Program {
    /// Creates a program,
    ///
    pub async fn create(storage: Shared) -> anyhow::Result<Self> {
        if let Some(mut node) = storage.current_resource::<ParsedNode>(ResourceKey::root()) {
            node.upgrade_node(&storage).await?;

            Ok(Program { node, storage })
        } else {
            Err(anyhow!("Could not create program, missing parsed node"))
        }
    }

    /// Returns the thunk context for this program,
    ///
    pub fn context(&self) -> anyhow::Result<ThunkContext> {
        let mut tc = ThunkContext::from(self.storage.clone().into_thread_safe());
        tc.set_attribute(self.node.node);
        Ok(tc)
    }
}
