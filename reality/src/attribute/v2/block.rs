use specs::HashMapStorage;
use specs::Component;
use toml_edit::Item;
use toml_edit::Document;
use super::ValueProvider;
use super::Attribute;

/// Struct representing a .runmd block,
/// 
#[derive(Component, Default)]
#[storage(HashMapStorage)]
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

    pub fn add_root(&mut self) {
        
    }
}

impl<'a> core::ops::Index<&'a str> for Block {
    type Output = Item;

    fn index(&self, index: &'a str) -> &Self::Output {
        &self.toml[index]
    }
}

impl ValueProvider<'_> for Block {}

