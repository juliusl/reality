use std::collections::{BTreeMap, BTreeSet};

use atlier::system::Value;
use specs::{Component, VecStorage};

/// Wrapper type for a collection of block property attributes
///
#[derive(Component, Debug, Default, Clone)]
#[storage(VecStorage)]
pub struct BlockProperties {
    map: BTreeMap<String, BlockProperty>,
}

/// Enumeration of property types
///
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockProperty {
    /// Property is a single value
    Single(Value),
    /// Property is a list of values
    List(Vec<Value>),
    Empty,
}

impl Default for BlockProperty {
    fn default() -> Self {
        BlockProperty::Empty
    }
}

impl BlockProperties {
    /// Adds a new property to the collection
    ///
    pub fn add(&mut self, name: impl AsRef<str>, value: impl Into<Value>) {
        match self.map.get_mut(name.as_ref()) {
            Some(existing) => match existing {
                BlockProperty::Single(val) => {
                    *existing = BlockProperty::List(vec![val.clone(), value.into()]);
                }
                BlockProperty::List(values) => {
                    values.push(value.into());
                }
                BlockProperty::Empty => {
                    *existing = BlockProperty::Single(value.into());
                }
            },
            None => {
                self.map.insert(
                    name.as_ref().to_string(),
                    BlockProperty::Single(value.into()),
                );
            }
        }
    }

    /// Sets a property
    ///
    pub fn set(&mut self, name: impl AsRef<str>, property: BlockProperty) {
        self.map.insert(name.as_ref().to_string(), property);
    }

    /// Returns values by property name
    ///
    pub fn property(&self, name: impl AsRef<str>) -> Option<&BlockProperty> {
        self.map.get(name.as_ref())
    }

    /// Returns a filtered set of properties using a `complex`
    /// 
    pub fn complex(&self, complex: &BTreeSet<String>) -> Option<Self> {
        let mut properties = BlockProperties::default();

        for k in complex.iter() {
            if let Some(property) = self.map.get(k) {
                properties.set(&k, property.clone());
            } else {
                properties.add(&k, Value::Empty);
            }
        }

        if !properties.map.is_empty() {
            Some(properties)
        } else {
            None
        }
    }
}
