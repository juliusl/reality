use super::Action;
use super::Attribute;
use super::Identifier;
use crate::Value;
use specs::Component;
use specs::HashMapStorage;

/// Struct representing a .runmd block,
///
#[derive(Component, Default)]
#[storage(HashMapStorage)]
pub struct Block {
    /// Identifier,
    /// 
    /// The root segment, pos(0) of the identifier is the family name, pos(1) is the name of the block,
    ///
    ident: Identifier,
    /// Initialization actions,
    /// 
    initialize: Vec<Action>,
    /// Block attributes,
    ///
    attributes: Vec<Attribute>,
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Block")
            .field("attributes", &self.attributes)
            .finish()
    }
}

impl Block {
    /// Returns a new block from identifier,
    ///
    pub fn new(ident: Identifier) -> Self {
        Self {
            ident,
            initialize: vec![],
            attributes: vec![],
        }
    }

    /// Returns an iterator over extensions this block requires,
    ///
    pub fn requires(&self) -> impl Iterator<Item = &Action> {
        self.attributes.iter().flat_map(|a| a.requires())
    }

    /// Returns the last attribute,
    ///
    pub fn last_mut(&mut self) -> Option<&mut Attribute> {
        self.attributes.last_mut()
    }

    /// Pushs an initialization action for this block,
    /// 
    pub fn initialize_with(&mut self, action: Action) {
        self.initialize.push(action);
    }

    /// Adds an attribute to the block,
    ///
    pub fn add_attribute(&mut self, ident: Identifier, value: impl Into<Value>) {
        self.attributes.push(Attribute::new(ident, value));
    }

    /// Returns the block family name,
    ///
    pub fn family(&self) -> String {
        self.ident.pos(0).unwrap_or_default()
    }

    /// Returns the block name,
    ///
    pub fn name(&self) -> Option<String> {
        if self.ident.len() < 1 {
            None
        } else {
            self.ident.pos(1).ok()
        }
    }
}
