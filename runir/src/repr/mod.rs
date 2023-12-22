pub(crate) mod dependency;
pub(crate) mod field;
pub(crate) mod host;
pub(crate) mod node;
pub(crate) mod recv;
pub(crate) mod resource;

pub mod prelude {
    pub use super::Repr;

    pub use super::resource::ResourceLevel;
    pub use super::resource::ResourceRepr;

    pub use super::field::Field;
    pub use super::field::FieldLevel;
    pub use super::field::FieldRepr;

    pub use super::recv::Recv;
    pub use super::recv::RecvLevel;
    pub use super::recv::RecvRepr;

    pub use super::node::NodeLevel;
    pub use super::node::NodeRepr;

    pub use super::dependency::DependencyLevel;
    pub use super::dependency::DependencyRepr;

    pub use super::host::HostLevel;
    pub use super::host::HostRepr;
}

use crate::define_intern_table;
use crate::prelude::*;
use anyhow::anyhow;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

use self::host::HostRepr;

// Intern table for intern handles
define_intern_table!(HANDLES: InternHandle);

// /// TODO (Phase1 - Bootstrap): This should end up replacing both block_info and node_info,
// ///
// /// Parsing is converting SourceLevel -> ResourceLevel?
// ///
// pub struct SourceLevel {
// }

/// Struct containing the tail reference of the representation,
///
/// A repr is a linked list of intern handle nodes that can unravel back into
/// a repr factory. This allows the repr to store and pass around a single u64 value
/// which can be used to query interned tags from each level.
///
#[derive(
    Hash, Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Repr {
    /// Tail end of the linked list,
    ///
    pub(crate) tail: InternHandle,
}

impl From<u64> for Repr {
    fn from(value: u64) -> Self {
        Repr {
            tail: InternHandle::from(value),
        }
    }
}

impl Repr {
    /// Returns as a u64 value,
    ///
    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.tail.as_u64()
    }

    /// Upgrades a representation in place w/ a new level,
    ///
    pub async fn upgrade(
        &mut self,
        mut interner: impl InternerFactory,
        level: impl Level,
    ) -> anyhow::Result<()> {
        // Configure a new handle
        let handle = level.configure(&mut interner).wait_for_ready().await;

        // TODO -- error handling
        // 1) Need verify the interner factory is the same as what was previously used
        // 2) Need to verify the next level is indeed the next level

        let to = Tag::new(&HANDLES, Arc::new(handle));

        let mut from = self.tail.clone();
        from.link = 0;

        let _ = Tag::new(&HANDLES, Arc::new(from)).link(&to).await?;

        if let Some(tail) = HANDLES.copy(&to.create_value.clone()).await {
            self.tail = tail;
            Ok(())
        } else {
            Err(anyhow!("Could not upgrade representation"))
        }
    }

    /// Return a vector containing an intern handle pointing to each level of this representation,
    ///
    /// The vector is ordered w/ the first element as the root and the last as the tail.
    ///
    pub fn try_get_levels(&self) -> Vec<InternHandle> {
        let mut levels = vec![];
        let mut cursor = self.tail.node();
        loop {
            match cursor {
                (Some(prev), current) => {
                    let prev = HANDLES.try_copy(&prev).unwrap();
                    levels.push(current);
                    cursor = prev.node();
                }
                (None, current) => {
                    levels.push(current);
                    levels.reverse();
                    return levels;
                }
            }
        }
    }

    /// Returns the repr as a resource repr,
    ///
    #[inline]
    pub fn as_resource(&self) -> Option<ResourceRepr> {
        self.try_get_levels().get(0).copied().map(ResourceRepr)
    }

    /// Returns the repr as a dependency repr,
    ///
    #[inline]
    pub fn as_dependency(&self) -> Option<DependencyRepr> {
        // TODO: Check if this is actually DependencyLevel?
        self.try_get_levels().get(1).copied().map(DependencyRepr)
    }

    /// Returns the repr as a receiver repr,
    ///
    #[inline]
    pub fn as_recv(&self) -> Option<RecvRepr> {
        self.try_get_levels().get(1).copied().map(RecvRepr)
    }

    /// Returns the repr as a field repr,
    ///
    #[inline]
    pub fn as_field(&self) -> Option<FieldRepr> {
        self.try_get_levels().get(1).copied().map(FieldRepr)
    }

    /// Returns the repr as a node repr,
    ///
    #[inline]
    pub fn as_node(&self) -> Option<NodeRepr> {
        self.try_get_levels().get(2).copied().map(NodeRepr)
    }

    /// Returns the repr as a host repr,
    ///
    #[inline]
    pub fn as_host(&self) -> Option<HostRepr> {
        self.try_get_levels().get(3).copied().map(HostRepr)
    }
}
