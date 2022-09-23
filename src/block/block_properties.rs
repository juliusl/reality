use std::collections::BTreeMap;

use atlier::system::Value;

/// Wrapper type for a collection of block property attributes
///
#[derive(Debug, Default, Clone)]
pub struct BlockProperties(BTreeMap<String, BlockProperty>);

/// Enumeration of property types
/// 
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockProperty {
    Single(Value),
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
        match self.0.get_mut(name.as_ref()) {
            Some(existing) => match existing {
                BlockProperty::Single(val) => {
                    *existing = BlockProperty::List(
                        vec![
                            val.clone(), 
                            value.into()
                        ]);
                }
                BlockProperty::List(values) => {
                    values.push(value.into());
                }
                BlockProperty::Empty => {
                    *existing = BlockProperty::Single(value.into());
                }
            },
            None => {
                self.0.insert(
                    name.as_ref().to_string(),
                    BlockProperty::Single(value.into()),
                );
            }
        }
    }

    /// Sets a property
    /// 
    pub fn set(&mut self, name: impl AsRef<str>, property: BlockProperty) {
        self.0.insert(name.as_ref().to_string(), property);
    }

    /// Returns values by property name
    ///
    pub fn property(&self, name: impl AsRef<str>) -> Option<&BlockProperty> {
        self.0.get(name.as_ref())
    }
}
