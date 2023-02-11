use std::collections::{HashMap, BTreeSet};

use crate::Value;
use tracing::{event, Level};

/// Struct to wrap interned data 
/// 
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interner {
    /// Strings that have been interned
    /// 
    strings: InternedStrings,
    /// A complex is a vector of strings that have been interned
    /// 
    complexes: InternedComplexes
}

/// Type alias for interned strings 
/// 
pub type InternedStrings = HashMap<u64, String>;

/// Type alias for interned complexes
/// 
pub type InternedComplexes = HashMap<u64, BTreeSet<String>>;

impl Interner {
    /// Returns the key for this ident,
    /// 
    pub fn ident(&self, ident: impl AsRef<str>) -> u64 {
        let ident = Value::Symbol(ident.as_ref().to_string());
        if let (Value::Reference(key), Value::Symbol(_)) = (ident.to_ref(), ident) {
            key 
        } else {
            event!(Level::ERROR, "Could not add string to interner");
            0
        }
    }

    /// Adds an ident to the interner
    /// 
    pub fn add_ident(&mut self, ident: impl AsRef<str>) -> u64 {
        let ident = Value::Symbol(ident.as_ref().to_string());
        if let (Value::Reference(key), Value::Symbol(ident)) = (ident.to_ref(), ident) {
            self.insert_string(key, ident);
            key 
        } else {
            event!(Level::ERROR, "Could not add string to interner");
            0
        }
    }

    /// Adds a map to the interner
    /// 
    pub fn add_map(&mut self, map: Vec<&str>) -> u64 {
        let complex = Value::Complex(BTreeSet::from_iter(map.iter().map(|m| m.to_string())));
        if let (Value::Reference(key), Value::Complex(complex)) = (complex.to_ref(), complex) {
            self.insert_complex(key, &complex);
            key
        } else {
            event!(Level::ERROR, "Could not add map to interner");
            0
        }
    }

    /// Adds a string to the interner w/ key value
    /// 
    pub fn insert_string(&mut self, key: u64, string: String) {
        self.strings.insert(key, string);
    }

    /// Adds a complex to the interner w/ key value
    /// 
    pub fn insert_complex(&mut self, key: u64, complex: &BTreeSet<String>) {
        self.complexes.insert(key, complex.to_owned());
    }

    /// Returns a reference to interned strings
    /// 
    pub fn strings(&self) -> &InternedStrings {
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

        for (_, s) in other.strings.iter() {
            interner.add_ident(s);
        }

        for (k, c) in other.complexes.iter() {
            interner.insert_complex(*k, &c);
        }

        interner
    }
}

impl Default for Interner {
    fn default() -> Self {
        let mut interner = Self { strings: Default::default(), complexes: Default::default() };
        // When this is converted into a control device, since a read must be > 0, this can't normally be encoded
        // So by default add this to the interner as a special case
        interner.add_ident("");
        interner
    }
}

impl From<InternedStrings> for Interner {
    fn from(strings: InternedStrings) -> Self {
        Self {
            strings,
            complexes: HashMap::default()
        }
    }
}

impl AsRef<InternedStrings> for Interner {
    fn as_ref(&self) -> &InternedStrings {
        &self.strings
    }
}

impl AsRef<InternedComplexes> for Interner {
    fn as_ref(&self) -> &InternedComplexes {
        &&self.complexes
    }
}

impl Into<HashMap<u64, String>> for Interner {
    fn into(self) -> HashMap<u64, String> {
        self.strings
    }
}
