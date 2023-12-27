use anyhow::anyhow;

use crate::Attribute;
use crate::ParsedNode;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;
use crate::ThunkContext;

use super::package::ProgramMatch;

/// Wrapper struct over a parsed node and it's corresponding storage,
///
#[derive(Clone)]
pub struct Program {
    /// Parsed node data,
    ///
    pub node: ParsedNode,
    /// Shared storage,
    ///
    storage: Shared,
    /// Entry point,
    ///
    entry_point: Option<ResourceKey<Attribute>>,
}

impl Program {
    /// Creates a program,
    ///
    pub async fn create(mut storage: Shared) -> anyhow::Result<Self> {
        if let Some(mut node) = storage.current_resource::<ParsedNode>(ResourceKey::root()) {
            node.upgrade_node(&storage).await?;

            // Important to note here, parsed node is never mutated outside of this
            storage.create_soft_links(&node);

            Ok(Program {
                node,
                storage,
                entry_point: None,
            })
        } else {
            Err(anyhow!("Could not create program, missing parsed node"))
        }
    }

    /// Returns the thunk context for this program,
    ///
    pub fn context(&self) -> anyhow::Result<ThunkContext> {
        let mut tc = ThunkContext::from(self.storage.clone().into_thread_safe());
        tc.set_attribute(self.entry_point.unwrap_or(self.node.node));
        Ok(tc)
    }

    /// Returns any node's whose paths end w/ name,
    ///    
    /// **Note** If `*` is used all programs w/ addresses are returned.
    ///
    pub fn search(&self, name: impl AsRef<str>) -> Vec<ProgramMatch> {
        let mut matches = vec![];
        for a in self.node.attributes.iter() {
            if let Some(host) = a.host() {
                if let Some(address) = host.try_address() {
                    let is_match = address.ends_with(name.as_ref()) || name.as_ref() == "*";
                    if is_match {
                        let mut program = self.clone();
                        program.entry_point = Some(*a);
                        matches.push(ProgramMatch { host, program });
                    }
                }
            }
        }
        matches
    }
}
