use crate::Error;
use crate::Identifier;
use crate::Value;
use specs::Component;
use specs::VecStorage;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::Index;
use std::ops::IndexMut;
use std::sync::Arc;

mod property;
pub use property::property_list;
pub use property::property_value;
pub use property::Property;

use super::data::query::Predicate;
use super::data::query::Query;
use super::data::query::QueryResult;
use super::Visitor;

/// Component for a map of properties,
///
#[derive(Component, Hash, Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[storage(VecStorage)]
pub struct Properties {
    /// Identifier of owner,
    ///
    owner: Identifier,
    /// Map of properties,
    ///
    map: BTreeMap<String, Property>,
}

impl Properties {
    /// Returns an empty properties map,
    ///
    pub const fn empty() -> Self {
        Self {
            owner: Identifier::new(),
            map: BTreeMap::new(),
        }
    }

    /// Branches this colelction w/ a new branch owner,
    ///
    pub fn branch(
        self: Arc<Self>,
        branch_owner: impl TryInto<Identifier, Error = Error>,
        property: Option<Property>,
    ) -> Result<Self, Error> {
        let mut properties = Properties::new(branch_owner.try_into()?);
        if let Some(property) = property {
            let subject = properties.owner().subject();
            properties[&subject] = property;
        }

        for (name, prop) in self.iter_properties() {
            properties[name] = prop.clone();
        }

        Ok(properties)
    }

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
                self.map
                    .insert(name.as_ref().to_string(), Property::Single(value.into()));
            }
        }
    }

    /// Add's a readonly block properties reference,
    ///
    pub fn add_readonly_properties(&mut self, properties: &Properties) {
        let properties = Arc::new(properties.clone());

        self.map.insert(
            format!("{}", properties.owner().subject()),
            Property::Properties(properties),
        );
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

    /// Returns true if this map contains a property w/ name,
    ///
    pub fn contains(&self, name: impl AsRef<str>) -> bool {
        self.map.contains_key(name.as_ref())
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

    /// Returns a property as a Properties map, can be used to extend a property
    ///
    pub fn extend_property(&self, name: impl AsRef<str>) -> Option<Properties> {
        self.owner().branch(name.as_ref()).ok().and_then(|owner| {
            self.map.get(name.as_ref()).map(move |p| {
                let mut properties = Properties::new(owner);
                properties[name.as_ref()] = p.clone();
                properties
            })
        })
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

    /// Queries all nested properties, non-recursive
    ///
    pub fn query_nested(
        &self,
        pat: impl Into<String>,
        predicate: impl Predicate + 'static,
    ) -> Vec<QueryResult> {
        let mut results = vec![];
        let pat = pat.into();
        for p in self.iter_properties().filter_map(|p| p.1.as_properties()) {
            if let Ok(result) = p.query(&pat, predicate) {
                for r in result {
                    results.push(r);
                }
            }
        }

        results
    }

    /// Shortcut for query_nested(.., all)
    ///
    pub fn all_nested(&self, pat: impl Into<String>) -> Vec<QueryResult> {
        self.query_nested(pat, super::data::query::all)
    }

    /// Returns the number of properties contained in the property map,
    ///
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

impl<'a> Index<&'a str> for Properties {
    type Output = Property;

    fn index(&self, index: &'a str) -> &Self::Output {
        self.property(index).unwrap_or(&Property::Empty)
    }
}

impl<'a> IndexMut<&'a str> for Properties {
    fn index_mut(&mut self, index: &'a str) -> &mut Self::Output {
        if !self.contains(index) {
            self.add(index, Value::Empty);
        }

        self.property_mut(index).expect("should exist just added")
    }
}

impl Visitor for Properties {
    fn visit_properties(&mut self, properties: &Properties) {
        self.add_readonly_properties(properties);
    }

    fn visit_property(&mut self, name: &str, property: &Property) {
        self.set(name, property.clone());
    }

    fn visit_symbol(&mut self, name: &str, idx: Option<usize>, symbol: &String) {
        if !self.contains(name) {
            self.add(name, symbol);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::Symbol(s)) => {
                s.visit_symbol(name, idx, symbol);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::Symbol(s)) = list.get_mut(idx.expect("should exist")) {
                    s.visit_symbol(name, idx, symbol);
                }
            }
            _ => {}
        });
    }

    fn visit_bool(&mut self, name: &str, idx: Option<usize>, bool: bool) {
        if !self.contains(name) {
            self.add(name, bool);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::Bool(b)) => {
                b.visit_bool(name, idx, bool);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::Bool(b)) = list.get_mut(idx.expect("should exist")) {
                    b.visit_bool(name, idx, bool);
                }
            }
            _ => {}
        });
    }

    fn visit_text_buffer(&mut self, name: &str, idx: Option<usize>, text_buffer: &String) {
        if !self.contains(name) {
            self.add(name, text_buffer);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::TextBuffer(t)) => {
                t.visit_text_buffer(name, idx, text_buffer);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::TextBuffer(t)) = list.get_mut(idx.expect("should exist")) {
                    t.visit_text_buffer(name, idx, text_buffer);
                }
            }
            _ => {}
        });
    }

    fn visit_int(&mut self, name: &str, idx: Option<usize>, i: i32) {
        if !self.contains(name) {
            self.add(name, i);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::Int(_i)) => {
                _i.visit_int(name, idx, i);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::Int(_i)) = list.get_mut(idx.expect("should exist")) {
                    _i.visit_int(name, idx, i);
                }
            }
            _ => {}
        });
    }

    fn visit_int_pair(&mut self, name: &str, idx: Option<usize>, pair: &[i32; 2]) {
        if !self.contains(name) {
            self.add(name, pair);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::IntPair(a, b)) => {
                a.visit_int(name, idx, pair[0]);
                b.visit_int(name, idx, pair[1]);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::IntPair(a, b)) = list.get_mut(idx.expect("should exist")) {
                    a.visit_int(name, idx, pair[0]);
                    b.visit_int(name, idx, pair[1]);
                }
            }
            _ => {}
        });
    }

    fn visit_int_range(&mut self, name: &str, idx: Option<usize>, range: &[i32; 3]) {
        if !self.contains(name) {
            self.add(name, range);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::IntRange(a, b, c)) => {
                a.visit_int(name, idx, range[0]);
                b.visit_int(name, idx, range[1]);
                c.visit_int(name, idx, range[2]);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::IntRange(a, b, c)) = list.get_mut(idx.expect("should exist")) {
                    a.visit_int(name, idx, range[0]);
                    b.visit_int(name, idx, range[1]);
                    c.visit_int(name, idx, range[2]);
                }
            }
            _ => {}
        });
    }

    fn visit_float(&mut self, name: &str, idx: Option<usize>, f: f32) {
        if !self.contains(name) {
            self.add(name, f);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::Float(_f)) => {
                _f.visit_float(name, idx, f);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::Float(_f)) = list.get_mut(idx.expect("should exist")) {
                    _f.visit_float(name, idx, f);
                }
            }
            _ => {}
        });
    }

    fn visit_float_pair(&mut self, name: &str, idx: Option<usize>, pair: &[f32; 2]) {
        if !self.contains(name) {
            self.add(name, pair);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::FloatPair(a, b)) => {
                a.visit_float(name, idx, pair[0]);
                b.visit_float(name, idx, pair[1]);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::FloatPair(a, b)) = list.get_mut(idx.expect("should exist")) {
                    a.visit_float(name, idx, pair[0]);
                    b.visit_float(name, idx, pair[1]);
                }
            }
            _ => {}
        });
    }

    fn visit_float_range(&mut self, name: &str, idx: Option<usize>, range: &[f32; 3]) {
        if !self.contains(name) {
            self.add(name, range);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::FloatRange(a, b, c)) => {
                a.visit_float(name, idx, range[0]);
                b.visit_float(name, idx, range[1]);
                c.visit_float(name, idx, range[2]);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::FloatRange(a, b, c)) = list.get_mut(idx.expect("should exist")) {
                    a.visit_float(name, idx, range[0]);
                    b.visit_float(name, idx, range[1]);
                    c.visit_float(name, idx, range[2]);
                }
            }
            _ => {}
        });
    }

    fn visit_binary(&mut self, name: &str, idx: Option<usize>, binary: &Vec<u8>) {
        if !self.contains(name) {
            self.add(name, binary);
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::BinaryVector(b)) => {
                b.visit_binary(name, idx, binary);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::BinaryVector(b)) = list.get_mut(idx.expect("should exist")) {
                    b.visit_binary(name, idx, binary);
                }
            }
            _ => {}
        });
    }

    fn visit_reference(&mut self, name: &str, idx: Option<usize>, reference: u64) {
        self.property_mut(name).map(|p| match p {
            Property::Single(Value::Reference(r)) => {
                r.visit_reference(name, idx, reference);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::Reference(r)) = list.get_mut(idx.expect("should exist")) {
                    r.visit_reference(name, idx, reference);
                }
            }
            _ => {}
        });
    }

    fn visit_complex(&mut self, name: &str, idx: Option<usize>, complex: &std::collections::BTreeSet<String>) {
        if !self.contains(name) {
            self.add(name, complex.clone());
            return;
        }

        self.property_mut(name).map(|p| match p {
            Property::Single(Value::Complex(c)) => {
                c.visit_complex(name, idx, complex);
            }
            Property::List(list) if idx.is_some() => {
                if let Some(Value::Complex(c)) = list.get_mut(idx.expect("should exist")) {
                    c.visit_complex(name, idx, complex);
                }
            }
            _ => {}
        });
    }
}

impl Display for Properties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Owner: {} -- {:#}", self.owner(), self.owner())?;
        writeln!(f, "Root: {:#}", self.owner().root())?;
        writeln!(f, "Subject: {:#}", self.owner().subject())?;
        for (name, property) in self.iter_properties() {
            if let Some(props) = property.as_properties() {
                writeln!(f, "\t{name}: {}", props)?;
            } else {
                writeln!(f, "\t{name}: {:?}", property)?;
            }
        }

        Ok(())
    }
}

#[allow(unused_imports)]
mod tests {
    use super::Properties;
    use crate::{
        v2::{properties::property::property_value, thunk_update, Property, Visitor},
        Identifier, Value,
    };

    #[test]
    fn test_properties_indexer() {
        let mut properties = Properties::new(Identifier::default());
        properties.add("test", "test-symbol");
        assert_eq!("test-symbol", properties["test"].as_symbol().unwrap());

        properties.add("test", "test-symbol-2");
        assert_eq!(
            "test-symbol-2",
            &properties["test"][1].symbol().unwrap_or_default()
        );

        let mut _inner = Properties::new("testa".parse().expect("should parse"));
        _inner.add("test-symbol-a", "test-symbol-a");
        properties.add_readonly_properties(&_inner);
        assert_eq!(
            "test-symbol-a",
            properties["testa"]["test-symbol-a"].as_symbol().unwrap()
        );

        properties["test-mut"] = property_value("test-mut-value");
        assert_eq!(
            "test-mut-value",
            properties["test-mut"].as_symbol().unwrap()
        );

        // Test an extended property can still be used like a normal property
        let extended = properties.extend_property("test-mut").unwrap();
        properties.add_readonly_properties(&extended);

        assert_eq!(
            "test-mut-value",
            properties["test-mut"].as_symbol().unwrap()
        );

        let mut test_visitor = Properties::empty();
        test_visitor.visit_symbol("test-symbol-a", None, &String::from("test-symbol-a"));
        assert_eq!(
            "test-symbol-a",
            test_visitor["test-symbol-a"].as_symbol().unwrap()
        );

        test_visitor.visit_symbol("test-symbol-a", None, &String::from("test-symbol-b"));
        assert_eq!(
            "test-symbol-b",
            test_visitor["test-symbol-a"].as_symbol().unwrap()
        );
    }
}
