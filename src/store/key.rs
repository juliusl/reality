use crate::wire::{Frame, Interner};

/// Struct for a store key,
///
#[derive(Default, Debug, PartialEq, Eq, Hash, Clone)]
pub struct Key {
    /// Name key,
    ///
    name: u64,
    /// Symbol key,
    ///
    symbol: u64,
    /// Original frame,
    ///
    frame: Frame,
}

impl Key {
    /// Returns a new key from a frame,
    /// 
    pub fn new(frame: Frame) -> Self {
        Key { name: frame.name_key(), symbol: frame.symbol_key(), frame }
    }

    /// Returns the name,
    ///
    pub fn name<'a>(&'a self, interner: &'a Interner) -> Option<&'a String> {
        interner.strings().get(&self.name)
    }

    /// Returns the symbol,
    ///
    pub fn symbol<'a>(&'a self, interner: &'a Interner) -> Option<&'a String> {
        interner.strings().get(&self.symbol)
    }

    /// Returns the hash code for this store key,
    ///
    /// This is jsut key ^ symbol
    ///
    pub fn hash_code(&self) -> u64 {
        self.name ^ self.symbol
    }
}

