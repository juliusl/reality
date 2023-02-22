use std::sync::Arc;
use std::collections::BTreeMap;
use specs::VecStorage;
use specs::Component;
use crate::Value;

mod property;
pub use property::Property;

/// Component for a map of property attributes
///
#[derive(Component, Hash, Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[storage(VecStorage)]
pub struct Properties {
    /// Name of this map of properties,
    name: String,
    /// Map of properties
    map: BTreeMap<String, Property>,
}

impl Properties {
    /// Creates a new set of properties w/ name
    ///
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            map: BTreeMap::default(),
        }
    }

    /// Returns the name of the root attribute that owns these properties,
    /// 
    pub fn name(&self) -> &String {
        &self.name
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
        self.map.insert(properties.name().clone(), Property::Properties(properties));
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
