use std::collections::{HashMap, BTreeSet};

/// Struct to wrap interned data 
/// 
#[derive(Clone, Default)]
pub struct Interner {
    /// Strings that have been interned
    /// 
    pub strings: HashMap<u64, String>,
    /// A complex is a vector of strings that have been interned
    /// 
    pub complexes: HashMap<u64, BTreeSet<String>>
}

impl Interner {
    pub fn add_string(&mut self, key: u64, string: String) {
        self.strings.insert(key, string);
    }

    pub fn add_complex(&mut self, key: u64, complex: &BTreeSet<String>) {
        self.complexes.insert(key, complex.to_owned());
    }
}

impl AsRef<HashMap<u64, String>> for Interner {
    fn as_ref(&self) -> &HashMap<u64, String> {
        &self.strings
    }
}

impl Into<HashMap<u64, String>> for Interner {
    fn into(self) -> HashMap<u64, String> {
        self.strings
    }
}
