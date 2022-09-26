use std::collections::{BTreeMap, BTreeSet};

use atlier::system::Value;
use specs::{Component, VecStorage};

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

/// Enumeration of property types
///
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockProperty {
    /// Property is a single value
    Single(Value),
    /// Property is a list of values
    List(Vec<Value>),
    /// Reverse property that indicates this property name is required
    Required,
    /// Reverse property that indiciates this property name is required
    Optional,
    /// Indicates that this block property is currently empty
    Empty,
}

impl BlockProperty {
    /// Returns a string if the property is a single text buffer
    ///
    pub fn text(&self) -> Option<&String> {
        match self {
            BlockProperty::Single(Value::TextBuffer(text)) => Some(text),
            _ => None,
        }
    }

    /// Returns a string if the property is a single symbol
    ///
    pub fn symbol(&self) -> Option<&String> {
        match self {
            BlockProperty::Single(Value::Symbol(symbol)) => Some(symbol),
            _ => None,
        }
    }

    /// Returns a vector of strings if the property is a single text buffer,
    /// or if the property is a list of values, filters all text buffers
    ///
    pub fn text_vec(&self) -> Option<Vec<&String>> {
        match self {
            BlockProperty::Single(Value::TextBuffer(text)) => Some(vec![text]),
            BlockProperty::List(values) => Some(
                values
                    .iter()
                    .filter_map(|m| match m {
                        Value::TextBuffer(t) => Some(t),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    /// Returns a vector of strings if the property is a single symbol,
    /// or if the property is a list of values, filters all symbols
    ///
    pub fn symbol_vec(&self) -> Option<Vec<&String>> {
        match self {
            BlockProperty::Single(Value::Symbol(text)) => Some(vec![text]),
            BlockProperty::List(values) => Some(
                values
                    .iter()
                    .filter_map(|m| match m {
                        Value::Symbol(t) => Some(t),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    /// Returns a vector of integers if the property is a single int,
    /// or if the property is a list of values, filters all ints
    ///
    pub fn int_vec(&self) -> Option<Vec<&i32>> {
        match self {
            BlockProperty::Single(Value::Int(int)) => Some(vec![int]),
            BlockProperty::List(values) => Some(
                values
                    .iter()
                    .filter_map(|m| match m {
                        Value::Int(i) => Some(i),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    /// Returns a vector of integers if the property is a single int,
    /// or if the property is a list of values, filters all ints
    ///
    pub fn float_vec(&self) -> Option<Vec<&f32>> {
        match self {
            BlockProperty::Single(Value::Float(float)) => Some(vec![float]),
            BlockProperty::List(values) => Some(
                values
                    .iter()
                    .filter_map(|m| match m {
                        Value::Float(i) => Some(i),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }
}

impl Default for BlockProperty {
    fn default() -> Self {
        BlockProperty::Empty
    }
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
                BlockProperty::Empty | BlockProperty::Required | BlockProperty::Optional => {
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
        self.with(property, BlockProperty::Required)
    }

    /// Sets a optional flag on the property name
    ///
    pub fn optional(&self, property: impl AsRef<str>) -> Self {
        self.with(property, BlockProperty::Optional)
    }

    /// Queries a source for required/optional properties this collection has,
    ///
    /// Returns a result if all required properties are covered.
    ///
    pub fn query(&self, source: &BlockProperties) -> Option<BlockProperties> {
        let mut result = self.clone();

        for (name, property) in self.query_parameters() {
            match property {
                BlockProperty::Required => {
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
                BlockProperty::Optional => {
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
            BlockProperty::Required | BlockProperty::Optional => true,
            _ => false,
        })
    }

    /// Returns values by property name
    ///
    pub fn property(&self, name: impl AsRef<str>) -> Option<&BlockProperty> {
        self.map.get(name.as_ref())
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
            BlockProperty::Required | BlockProperty::Optional | BlockProperty::Empty => None,
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
