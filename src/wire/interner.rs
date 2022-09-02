use std::collections::HashMap;

/// Struct to wrap interned data 
/// 
pub struct Interner {
    /// Strings that have been interned 
    /// 
    pub strings: HashMap<u64, String>
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
