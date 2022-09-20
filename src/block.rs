use atlier::system::{Attribute, Value};
use std::collections::BTreeMap;
use specs::{Entity, Component};
use specs::storage::DefaultVecStorage;
use serde::{Serialize, Deserialize};

mod block_index;
pub use block_index::BlockIndex;

/// Data structure parsed from .runmd, 
/// 
/// Stores stable and transient attributes. Can be encoded into 
/// a frame, which is a wire protocol type. 
///
#[derive(Component, Hash, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Clone, Default, Debug)]
#[storage(DefaultVecStorage)]
pub struct Block {
    /// World identifier - assigned by the runtime
    entity: u32,
    /// Primary identifier - user/runtime assigned
    name: String, 
    /// Secondary identifier - user/runtime assigned 
    symbol: String, 
    /// Block state - current state of the block 
    attributes: Vec<Attribute>,
}

impl Block {
    /// Creates a new empty block
    ///
    pub fn new(entity: Entity, name: impl AsRef<str>, symbol: impl AsRef<str>) -> Self {
        Self {
            entity: entity.id(),
            name: name.as_ref().to_string(),
            symbol: symbol.as_ref().to_string(),
            attributes: vec![],
        }
    }

    /// Returns a vector of block indexes,
    /// 
    pub fn index(&self) -> Vec<BlockIndex> {
        BlockIndex::index(self)
    }

    /// Returns true if the entity is 0, 
    /// 
    /// **Note** The root block must always be entity 0. 
    /// 
    pub fn is_root_block(&self) -> bool {
        self.entity == 0
    }

    /// Returns the block name
    ///
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Returns the block symbol
    ///
    pub fn symbol(&self) -> &String {
        &self.symbol
    }

    /// Returns the entity id for the block
    /// 
    pub fn entity(&self) -> u32 {
        self.entity
    }

    /// Adds an attribute to the block, 
    /// 
    /// **Caveat** If an attribute already exists w/ the same 
    /// name, the last attribute added will be used as the primary attribute. 
    ///
    pub fn add_attribute(&mut self, attr: &Attribute) {
        let mut attr = attr.clone();
        attr.id = self.entity;
        self.attributes.push(attr);
    }

    /// Returns a block index from a map of transient attributes,
    /// 
    pub fn block_index(&self, prefix: impl AsRef<str>) -> BlockIndex {
        let property_map = self.map_transient(prefix);

        BlockIndex::from(property_map)
    }

    /// Map transient values w/ prefix,
    ///
    /// Returns a map where the key is the name of the attribute w/o the prefix
    /// and the transient value.
    ///
    /// TODO: Need to handle multiple stable attribute copies,
    /// 
    pub fn map_transient(&self, prefix: impl AsRef<str>) -> BTreeMap<String, Value> {
        let mut map = BTreeMap::new();

        for (name, value) in self
            .attributes
            .iter()
            .filter(|a| !a.is_stable() && a.name.starts_with(prefix.as_ref()))
            .filter_map(|a| a.transient())
        {
            map.insert(
                name.trim_start_matches(prefix.as_ref())
                    .trim_start_matches("::")
                    .to_string(),
                value.clone(),
            );
        }

        map
    }

    /// Map all stable values,
    /// 
    /// Returns a map of attribute name's and values
    /// 
    pub fn map_stable(&self) -> BTreeMap<String, Value> {
        let mut map = BTreeMap::new();
        for (name, value) in self
            .attributes
            .iter()
            .filter(|a| a.is_stable())
            .map(|a| (&a.name, &a.value))
        {
            map.insert(name.to_string(), value.clone());
        }

        map
    }

    /// Map all control values,
    /// 
    /// Control values are transient attributes declared with the `::` operator, 
    /// within a block context before any stable attribute is declared.
    /// 
    /// Mechanically this means the transient map w/ the block symbol as the prefix.
    /// 
    pub fn map_control(&self) -> Option<BTreeMap<String, Value>> {
        if self.name.is_empty() && !self.symbol.is_empty() {
            Some(self.map_transient(&self.symbol))
        } else {
            None 
        }
    }

    /// Returns an iterator over all attributes,
    ///
    pub fn iter_attributes(&self) -> impl Iterator<Item = &Attribute> {
        self.attributes.iter().rev()
    }

    /// Returns an iterator over all transient values,
    ///
    pub fn iter_transient_values(&self) -> impl Iterator<Item = &(String, Value)> {
        self.attributes.iter().filter_map(|a| a.transient())
    }
}
