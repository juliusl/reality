use std::ops::Index;
use std::sync::Arc;
use std::collections::BTreeMap;
use specs::VecStorage;
use specs::Component;
use crate::Identifier;
use crate::Value;

mod property;
pub use property::Property;

/// Component for a map of property attributes
///
#[derive(Component, Hash, Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[storage(VecStorage)]
pub struct Properties {
    /// Identifier of owner,
    owner: Identifier,
    /// Map of properties
    map: BTreeMap<String, Property>,
}

impl Properties {
    /// Creates a new set of properties w/ name
    ///
    pub fn new(owner: Identifier) -> Self {
        Self {
            owner,
            map: BTreeMap::default(),
        }
    }

    /// Returns the name of the root attribute that owns these properties,
    /// 
    pub fn owner(&self) -> &Identifier {
        &self.owner
    }

    /// Adds a new property value to the collection,
    ///
    pub fn add(&mut self, name: impl AsRef<str>, value: impl Into<Value>) {
        match self.map.get_mut(name.as_ref()) {
            Some(existing) => match existing {
                Property::Single(val) => {
                    *existing = Property::List(vec![val.clone(), value.into()]);
                }
                Property::List(values) => {
                    values.push(value.into());
                }
                Property::Empty => {
                    *existing = Property::Single(value.into());
                }
                Property::Properties(_) => {
                    return;
                }
            },
            None => {
                self.map.insert(
                    name.as_ref().to_string(),
                    Property::Single(value.into()),
                );
            }
        }
    }

    /// Add's a readonly block properties reference,
    /// 
    pub fn add_readonly_properties(&mut self, properties: &Properties) {
        let properties = Arc::new(properties.clone());
        
        self.map.insert(format!("{:#}", properties.owner()), Property::Properties(properties));
    }

    /// Removes a property,
    /// 
    pub fn remove(&mut self, name: impl AsRef<str>) -> Option<Property> {
        self.map.remove(name.as_ref())
    }

    /// Sets a property
    ///
    pub fn set(&mut self, name: impl AsRef<str>, property: Property) {
        self.map.insert(name.as_ref().to_string(), property);
    }

    /// Returns a clone of self w/ property
    ///
    pub fn with(&self, name: impl AsRef<str>, property: Property) -> Self {
        let mut clone = self.clone();
        clone.set(name, property);
        clone
    }

    /// Returns values by property name
    ///
    pub fn property(&self, name: impl AsRef<str>) -> Option<&Property> {
        self.map.get(name.as_ref())
    }

    /// Returns mutable value by property name
    /// 
    pub fn property_mut(&mut self, name: impl AsRef<str>) -> Option<&mut Property> {
        self.map.get_mut(name.as_ref())
    }

    /// Takes a property from this collection, replaces with `Empty`
    ///
    pub fn take(&mut self, name: impl AsRef<str>) -> Option<Property> {
        self.map.get_mut(name.as_ref()).and_then(|b| match b {
            Property::Single(_) | Property::List(_) | Property::Properties(_) => {
                let taken = Some(b.clone());
                *b = Property::Empty;
                taken
            }
            Property::Empty => None,
        })
    }

    /// Returns an iterator over the current map state,
    ///
    pub fn iter_properties(&self) -> impl Iterator<Item = (&String, &Property)> {
        self.map.iter()
    }

    /// Returns a mutable iterator over the current map state,
    /// 
    pub fn iter_properties_mut(&mut self) -> impl Iterator<Item = (&String, &mut Property)> {
        self.map.iter_mut().map(|(name, property)| (name, property))
    }
}

impl<'a> Index<&'a str> for Properties {
    type Output = Property;

    fn index(&self, index: &'a str) -> &Self::Output {
        self.property(index).unwrap_or(&Property::Empty)
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::Identifier;
    use super::Properties;

    #[test]
    fn test_properties_indexer() {
        let mut properties = Properties::new(Identifier::default());
        properties.add("test", "test-symbol");
        assert_eq!("test-symbol", properties["test"].as_symbol().unwrap());

        properties.add("test", "test-symbol-2");
        assert_eq!("test-symbol-2", &properties["test"][1].symbol().unwrap_or_default());

        let mut _inner = Properties::new("testa".parse().expect("should parse"));
        _inner.add("test-symbol-a", "test-symbol-a");
        properties.add_readonly_properties(&_inner);
        assert_eq!("test-symbol-a", properties["testa"]["test-symbol-a"].as_symbol().unwrap());
    }
}