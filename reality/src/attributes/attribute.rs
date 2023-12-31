use crate::ResourceKey;

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
}
