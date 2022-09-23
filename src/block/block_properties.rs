use std::collections::BTreeMap;

use atlier::system::Value;

/// Wrapper type for a collection of block property attributes
/// 
#[derive(Debug, Default, Clone)]
pub struct BlockProperties(BTreeMap<String, Vec<Value>>);

impl BlockProperties {
    /// Adds a new property to the collection
    /// 
    pub fn add(&mut self, name: impl AsRef<str>, value: impl Into<Value>) {
        if let Some(props) = self.0.get_mut(name.as_ref()) {
            props.push(value.into());
        } else {
            self.0.insert(name.as_ref().to_string(), vec![value.into()]);
        }
    }

    /// Returns values by property name
    /// 
    pub fn property(&self, name: impl AsRef<str>) -> Option<&Vec<Value>> {
        self.0.get(name.as_ref())
    }
}
