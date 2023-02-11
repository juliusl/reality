use specs::{Component, VecStorage};
use toml_edit::{Document, Item};

use super::{Attribute, ValueProvider};

/// Struct representing a .runmd block,
/// 
#[derive(Component)]
#[storage(VecStorage)]
pub struct Block {
    /// Internal toml document compiled from .runmd block,
    /// 
    toml: Document,
    /// Attributes parsed from the document,
    /// 
    attributes: Vec<Attribute>,
}

impl Block {
    /// Returns the block name,
    /// 
    pub fn name(&self) -> Option<String> {
        self.toml["name"].as_str().map(|s| s.to_string())
    }

    /// Returns the block symbol,
    /// 
    pub fn symbol(&self) -> Option<String> {
        self.toml["symbol"].as_str().map(|s| s.to_string())
    }
}

impl<'a> core::ops::Index<&'a str> for Block {
    type Output = Item;

    fn index(&self, index: &'a str) -> &Self::Output {
        &self.toml[index]
    }
}

impl ValueProvider<'_> for Block {}

