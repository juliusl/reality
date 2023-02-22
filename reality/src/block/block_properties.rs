use std::collections::BTreeSet;
use std::collections::BTreeMap;

use crate::Value;
use specs::VecStorage;
use specs::Component;

mod block_property;
pub use block_property::BlockProperty;
pub use block_property::display_value;

mod documentation;
pub use documentation::Documentation;

/// Wrapper type for a collection of block property attributes
///
#[derive(Component, Hash, Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[storage(VecStorage)]
pub struct BlockProperties {
    /// Name of this map of properties, (complex attribute's name)
    name: String,
    /// Map of properties
    map: BTreeMap<String, BlockProperty>,
}

impl BlockProperties {
    /// Creates a new set of block properties w/ name
    ///
    pub fn new(name: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            map: BTreeMap::default(),
        }
    }

    /// Returns the name of the root attribute that owns these properties,
    /// 
    pub fn name(&self) -> &String {
        &self.name
    }

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
                BlockProperty::Empty | BlockProperty::Required(_) | BlockProperty::Optional(_) => {
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

    /// Removes a property,
    /// 
    pub fn remove(&mut self, name: impl AsRef<str>) -> Option<BlockProperty> {
        self.map.remove(name.as_ref())
    }

    /// Sets a property
    ///
    pub fn set(&mut self, name: impl AsRef<str>, property: BlockProperty) {
        self.map.insert(name.as_ref().to_string(), property);
    }

    /// Returns a clone of self w/ property
    ///
    pub fn with(&self, name: impl AsRef<str>, property: BlockProperty) -> Self {
        let mut clone = self.clone();
        clone.set(name, property);
        clone
    }

    /// Sets a required flag on the property name
    ///
    pub fn require(&self, property: impl AsRef<str>) -> Self {
        self.with(property, BlockProperty::Required(None))
    }

    /// Sets a optional flag on the property name
    ///
    pub fn optional(&self, property: impl AsRef<str>) -> Self {
        self.with(property, BlockProperty::Optional(None))
    }

    /// Sets a required flag on the property name with a default value,
    ///
    pub fn require_with(&self, property: impl AsRef<str>, default_value: Value) -> Self {
        self.with(property, BlockProperty::Required(Some(default_value)))
    }

    /// Sets a optional flag on the property name with a default value,
    ///
    pub fn optional_with(&self, property: impl AsRef<str>, default_value: Value) -> Self {
        self.with(property, BlockProperty::Optional(Some(default_value)))
    }

    /// Queries a source for required/optional properties this collection has,
    ///
    /// Returns a result if all required properties are covered.
    ///
    pub fn query(&self, source: &BlockProperties) -> Option<BlockProperties> {
        let mut result = self.clone();

        for (name, property) in self.query_parameters() {
            match property {
                BlockProperty::Required(_) => {
                    if let Some(required) = source.property(name) {
                        match required {
                            BlockProperty::Single(_) | BlockProperty::List(_) => {
                                result.set(name, required.clone());
                            }
                            _ => {
                                return None;
                            }
                        }
                    } else {
                        return None;
                    }
                }
                BlockProperty::Optional(_) => {
                    if let Some(optional) = source.property(name) {
                        match optional {
                            BlockProperty::Single(_) | BlockProperty::List(_) => {
                                result.set(name, optional.clone());
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                }
                _ => {
                    continue;
                }
            }
        }

        Some(result)
    }

    /// Gets query parameters found in this collection
    ///
    fn query_parameters(&self) -> impl Iterator<Item = (&String, &BlockProperty)> {
        self.map.iter().filter(|(_, prop)| match prop {
            BlockProperty::Required(_) | BlockProperty::Optional(_) => true,
            _ => false,
        })
    }

    /// Returns values by property name
    ///
    pub fn property(&self, name: impl AsRef<str>) -> Option<&BlockProperty> {
        self.map.get(name.as_ref())
    }

    /// Returns mutable value by property name
    /// 
    pub fn property_mut(&mut self, name: impl AsRef<str>) -> Option<&mut BlockProperty> {
        self.map.get_mut(name.as_ref())
    }

    /// Takes a property from this collection, replaces with `Empty`
    ///
    pub fn take(&mut self, name: impl AsRef<str>) -> Option<BlockProperty> {
        self.map.get_mut(name.as_ref()).and_then(|b| match b {
            BlockProperty::Single(_) | BlockProperty::List(_) => {
                let taken = Some(b.clone());
                *b = BlockProperty::Empty;
                taken
            }
            BlockProperty::Required(_) | BlockProperty::Optional(_) | BlockProperty::Empty => None,
        })
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

    /// Returns an iterator over the current map state,
    ///
    pub fn iter_properties(&self) -> impl Iterator<Item = (&String, &BlockProperty)> {
        self.map.iter()
    }

    /// Returns a mutable iterator over the current map state,
    /// 
    pub fn iter_properties_mut(&mut self) -> impl Iterator<Item = (&String, &mut BlockProperty)> {
        self.map.iter_mut().map(|(name, property)| (name, property))
    }
}

#[test]
fn test_block_properties() {
    let query = BlockProperties::default()
        .require("name")
        .require("type")
        .optional("enabled");

    let mut source_w_partial = BlockProperties::default();
    source_w_partial.add("name", "test");

    let mut source_w_all = source_w_partial.clone();
    source_w_all.add("type", "test-type");

    // Test query returns none if the source has partial requirements
    assert_eq!(query.query(&source_w_partial), None);

    // Test query returns some if source has all requirements
    assert!(query.query(&source_w_all).is_some());

    // Test query result has the correct property value
    assert_eq!(
        query.query(&source_w_all).unwrap().property("name"),
        Some(&BlockProperty::Single(Value::Symbol("test".to_string())))
    );

    // Test taking a property,
    assert_eq!(
        source_w_all.take("type"),
        Some(BlockProperty::Single(Value::Symbol(
            "test-type".to_string()
        )))
    );
}
