use std::collections::BTreeMap;

use atlier::system::{Attribute, Value};
use specs::Entity;

/// A block from .runmd delimitted w/ ```
///
#[derive(Default, Debug)]
pub struct Block {
    entity: u32,
    name: String,
    symbol: String,
    attributes: Vec<Attribute>,
}

impl Block {
    /// Creates a new block
    ///
    pub fn new(entity: Entity, name: impl AsRef<str>, symbol: impl AsRef<str>) -> Self {
        Self {
            entity: entity.id(),
            name: name.as_ref().to_string(),
            symbol: symbol.as_ref().to_string(),
            attributes: vec![],
        }
    }

    /// Adds a new attribute to the block
    ///
    pub fn add_attribute(&mut self, attr: &Attribute) {
        let mut attr = attr.clone();
        attr.id = self.entity;
        self.attributes.push(attr);
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

    /// Map transient values w/ prefix,
    ///
    /// Returns a map where the key is the name of the attribute w/o the prefix
    /// and the transient value.
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

    /// Returns an iterator over all attributes
    ///
    pub fn iter_attributes(&self) -> impl Iterator<Item = &Attribute> {
        self.attributes.iter()
    }

    /// Returns an iterator over all transient values
    ///
    pub fn iter_transient_values(&self) -> impl Iterator<Item = &(String, Value)> {
        self.attributes.iter().filter_map(|a| a.transient())
    }
}
