use std::collections::BTreeSet;
use std::collections::HashMap;
use crate::Error;
use crate::Value;

/// Struct to wrap interned data 
/// 
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interner {
    /// Symbols that have been interned,
    /// 
    /// Symbols are just string values that are assigned an intern-key and stored directly in frame state,
    /// rather than being stored in a blob device,
    /// 
    symbols: InternedSymbols,
    /// A complex is a vector of strings that have been interned
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
    /// Returns the key for this symbol,
    /// 
    pub fn symbol(&self, symbol: impl Into<String>) -> Result<u64, Error> {
        let symbol = Value::Symbol(symbol.into());
        if let (Value::Reference(key), Value::Symbol(_)) = (symbol.to_ref(), symbol) {
            Ok(key) 
        } else {
            Err("Could not add string to interner".into())
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

    /// Adds a map to the interner
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
