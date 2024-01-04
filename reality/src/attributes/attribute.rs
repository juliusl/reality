use std::{collections::BTreeMap, sync::Arc};

use crate::{FieldPacket, ResourceKey};

/// Pointer-struct representing the beginning a storage node,
///
#[derive(Hash, PartialEq, Eq, Debug, Default, Clone, Copy)]
pub struct Node;

/// Pointer-struct representing a parsed attribute,
///
#[derive(Hash, PartialEq, Eq, Debug, Default, Clone, Copy)]
pub struct Attribute;

/// Pointer-struct representing a defined property of a parsed attribute,
///
#[derive(Hash, PartialEq, Eq, Debug, Default, Clone, Copy)]
pub struct Property;

impl ResourceKey<Attribute> {
    /// Returns the value of a property set from annotations,
    ///
    pub fn prop(&self, name: impl AsRef<str>) -> Option<String> {
        self.node()
            .and_then(|n| n.annotations())
            .and_then(|a| a.get(name.as_ref()).cloned())
    }

    /// Returns annotations set on this node,
    ///
    pub fn annotations(&self) -> Option<Arc<BTreeMap<String, String>>> {
        self.node().and_then(|n| n.annotations())
    }

    /// Returns doc headers set on this node,
    ///
    pub fn doc_headers(&self) -> Option<Arc<Vec<String>>> {
        self.node().and_then(|n| n.doc_headers())
    }
}

impl ResourceKey<Property> {
    /// Returns an empty packet for this field,
    ///
    pub fn field_packet(&self) -> Option<FieldPacket> {
        self.empty_packet()
    }
}
