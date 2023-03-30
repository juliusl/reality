use std::collections::BTreeSet;
use std::collections::HashMap;
use std::ops::Deref;
use specs::Component;
use specs::VecStorage;
use tracing::error;
use crate::Error;
use crate::Value;

use crate::v2::states::Object;
use super::Properties;
use super::Visitor;

/// Struct w/ hash tables for interning types,
/// 
/// Interning is an encoding technique to map strings and other dynamically sized types to an integer value. 
/// When encoding, this integer value can be used in place of the actual instance value, and then restored later. 
/// 
/// Interners can be merged into a single Interner and encoded into a "Control Device",
/// 
#[derive(Component, Clone, Debug, PartialEq, Eq)]
#[storage(VecStorage)]
pub struct Interner {
    /// Symbols that have been interned,
    /// 
    /// Symbols are just string values that are assigned an intern-key and stored directly in frame state,
    /// rather than being stored in a blob device,
    /// 
    symbols: InternedSymbols,
    /// A complex is a set of strings that have been interned
    /// 
    complexes: InternedComplexes
}

/// Type alias for interned strings 
/// 
pub type InternedSymbols = HashMap<u64, String>;

/// Type alias for interned complexes
/// 
pub type InternedComplexes = HashMap<u64, BTreeSet<String>>;

impl Interner {
    /// Gets a value (either a symbol of complex) from an interner if present,
    /// 
    pub fn get(&self, key: &u64) -> Option<Value> {
        let mut value = self.get_symbol(key);
        
        if value == None {
            value = self.get_complex(key);
        }

        value
    }

    /// Gets a symbol from an interner if present,
    /// 
    pub fn get_symbol(&self, key: &u64) -> Option<Value> {
        self.symbols.get(key).map(|s| Value::Symbol(s.to_string()))
    }

    /// Gets a complex from the interner if present,
    /// 
    pub fn get_complex(&self, key: &u64) -> Option<Value> {
        self.complexes.get(key).map(|c| Value::Complex(c.clone()))
    }

    /// Returns the key for this symbol,
    /// 
    pub fn symbol(&self, symbol: impl Into<String>) -> Result<u64, Error> {
        let symbol = Value::Symbol(symbol.into());
        if let (Value::Reference(key), Value::Symbol(_)) = (symbol.to_ref(), symbol) {
            Ok(key) 
        } else {
            Err("Could not get key for symbol".into())
        }
    }

    /// Adds a symbol to the interner and returns the key,
    /// 
    pub fn add_symbol(&mut self, ident: impl AsRef<str>) -> Result<u64, Error> {
        let ident = Value::Symbol(ident.as_ref().to_string());
        if let (Value::Reference(key), Value::Symbol(ident)) = (ident.to_ref(), ident) {
            self.insert_symbol(key, ident);
            Ok(key)
        } else {
            Err("Could not add string to interner".into())
        }
    }

    /// Adds a map to the interner,
    /// 
    pub fn add_map(&mut self, map: Vec<&str>) -> Result<u64, Error> {
        let complex = Value::Complex(BTreeSet::from_iter(map.iter().map(|m| m.to_string())));
        if let (Value::Reference(key), Value::Complex(complex)) = (complex.to_ref(), complex) {
            self.insert_complex(key, &complex);
            Ok(key)
        } else {
            Err("Could not add map to interner".into())
        }
    }

    /// Adds a symbol to the interner w/ key value
    /// 
    pub fn insert_symbol(&mut self, key: u64, symbol: impl Into<String>) {
        self.symbols.insert(key, symbol.into());
    }

    /// Adds a complex to the interner w/ key value
    /// 
    pub fn insert_complex(&mut self, key: u64, complex: &BTreeSet<String>) {
        self.complexes.insert(key, complex.to_owned());
    }

    /// Returns a reference to interned symbols
    /// 
    pub fn symbols(&self) -> &InternedSymbols {
        self.as_ref()
    }
    
    /// Returns a reference to interned complexes
    /// 
    pub fn complexes(&self) -> &InternedComplexes {
        self.as_ref()
    }

    /// Merges two interners and returns a new interner,
    /// 
    pub fn merge(&self, other: &Interner) -> Interner {
        let mut interner = self.clone();

        for (k, s) in other.symbols.iter() {
            interner.insert_symbol(*k, s);
        }

        for (k, c) in other.complexes.iter() {
            interner.insert_complex(*k, &c);
        }

        interner
    }
}

impl Default for Interner {
    fn default() -> Self {
        let mut interner = Self { symbols: Default::default(), complexes: Default::default() };
        // When this is converted into a control device, since a read must be > 0, this can't normally be encoded
        // So by default add this to the interner as a special case
        interner.add_symbol("").ok();
        interner
    }
}

impl From<InternedSymbols> for Interner {
    fn from(strings: InternedSymbols) -> Self {
        Self {
            symbols: strings,
            complexes: HashMap::default()
        }
    }
}

impl AsRef<InternedSymbols> for Interner {
    fn as_ref(&self) -> &InternedSymbols {
        &self.symbols
    }
}

impl AsRef<InternedComplexes> for Interner {
    fn as_ref(&self) -> &InternedComplexes {
        &self.complexes
    }
}

impl Into<HashMap<u64, String>> for Interner {
    fn into(self) -> HashMap<u64, String> {
        self.symbols
    }
}

/// Allows the build itself to derive an interner,
/// 
impl Visitor for Interner {
    fn visit_property_name(&mut self, name: &String) {
        self.add_symbol(name).map_err(|e| error!("Could not add property name to interner, {e}")).ok();
    }

    fn visit_symbol(&mut self, _: &String, _: Option<usize>, symbol: &String) {
        self.add_symbol(symbol).map_err(|e| error!("Could not add symbol to interner, {e}")).ok();
    }

    fn visit_readonly(&mut self, properties: std::sync::Arc<Properties>) {
        let interner: Interner = properties.deref().into();

        *self = self.merge(&interner);
    }

    fn visit_identifier(&mut self, identifier: &crate::Identifier) {
        if let Some(parent) = identifier.parent() {
            self.add_symbol(format!("{:#}", parent)).map_err(|e| error!("Could not add parent identifier to interner, {e}")).ok();
        }

        self.add_symbol(format!("{}", identifier)).map_err(|e| error!("Could not add identifier to interner, {e}")).ok();

        match identifier.commit().and_then(|i| i.parts()) {
            Ok(parts) => {
                for p in parts.iter() {
                    self.add_symbol(p).map_err(|e| error!("Could not add identifier part to interner, {e}")).ok();
                }
            },
            Err(err) => {
                error!("Could not get parts from identifier, {err}");
            }
        }
    }
}

impl<'a> TryFrom<&Object<'a>> for Interner {
    type Error = Error;

    fn try_from(value: &Object<'a>) -> Result<Self, Self::Error> {
        let interner = Interner::default();
        let mut interner = interner.merge(&value.properties().into());
        
        value.ident().parts().map(|p| {
            p.iter().for_each(|p| {
                interner.add_symbol(p).ok();
            });
        })?;

        Ok(interner)
    }
}

impl From<Properties> for Interner {
    fn from(value: Properties) -> Self {
        (&value).into()
    }
}

impl From<&Properties> for Interner {
    fn from(value: &Properties) -> Self {
        let mut interner = Interner::default();

        for (name, property) in value.iter_properties() {
            interner.add_symbol(name).map_err(|e| {
                error!("Error adding property name to interner, {e}");
            }).ok();

            property.as_symbol().map(|p| {
                interner.add_symbol(p).map_err(|e| {
                    error!("Error adding symbol to interner, {e}");
                })
            });

            property.as_symbol_vec().map(|p| {
                for _p in p {
                    interner.add_symbol(_p).map_err(|e| {
                        error!("Error adding symbol to interner, {e}");
                    }).ok();
                }
            });

            property.as_properties().map(|p| {
                let nested: Interner = p.deref().into();

                interner = interner.merge(&nested);
            });
        }

        interner
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::v2::Properties;

    use super::Interner;

    #[test]
    fn test_interner_from_properties() {
        let mut properties = Properties::new("test".parse().expect("should be able to parse"));
        properties.add("a", "test");
        properties.add("b", "test2");
        properties.add("b", "test3");

        let mut nested = Properties::new("test".parse().expect("should be able to parse"));
        nested.add("c", "test4");
        nested.add("d", "test5");
        nested.add("d", "test6");
        properties.add_readonly_properties(&nested);

        let interner = Interner::try_from(properties).expect("should be able to convert interner");
        let key = interner.symbol("a").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
        let key = interner.symbol("b").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
        let key = interner.symbol("c").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
        let key = interner.symbol("d").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
        let key = interner.symbol("test").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
        let key = interner.symbol("test2").expect("should be able to get key");
        assert!(interner.get(&key).is_some(), "{:?}", interner);
        let key = interner.symbol("test3").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
        let key = interner.symbol("test4").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
        let key = interner.symbol("test5").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
        let key = interner.symbol("test6").expect("should be able to get key");
        assert!(interner.get(&key).is_some());
    }
}