use super::Action;
use super::Properties;
use super::Root;
use super::Build;
use crate::Identifier;
use crate::Value;
use specs::Builder;
use specs::Component;
use specs::HashMapStorage;

/// Struct representing a .runmd block,
///
#[derive(Component, Clone, Debug, Default)]
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
    /// Block roots,
    ///
    roots: Vec<Root>,
}

impl Block {
    /// Returns a new block from identifier,
    ///
    pub fn new(ident: Identifier) -> Self {
        Self {
            ident,
            initialize: vec![],
            roots: vec![],
        }
    }

    /// Returns an iterator over extensions this block requires,
    ///
    pub fn requires(&self) -> impl Iterator<Item = &Identifier> {
        self.roots.iter().flat_map(|a| a.extensions())
    }

    /// Returns a mutable reference to the last root,
    ///
    pub fn last_mut(&mut self) -> Option<&mut Root> {
        self.roots.last_mut()
    }

    /// Pushs an initialization action for this block,
    ///
    pub fn initialize_with(&mut self, action: Action) {
        self.initialize.push(action);
    }

    /// Adds an attribute to the block,
    ///
    pub fn add_root(&mut self, ident: Identifier, value: impl Into<Value>) {
        self.roots.push(Root::new(ident, value));
    }

    /// Returns an iterator over roots,
    ///
    pub fn roots(&self) -> impl Iterator<Item = &Root> {
        self.roots.iter()
    }

    /// Returns the root count,
    ///
    pub fn root_count(&self) -> usize {
        self.roots.len()
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

    /// Returns the ident for this block,
    ///
    pub fn ident(&self) -> &Identifier {
        &self.ident
    }

    /// Finalizes this block,
    ///
    pub fn finalize(&mut self) {
        self.ident.add_tag("block");
    }
}

impl Build for Block {
    fn build(
        &self,
        lazy_builder: specs::world::LazyBuilder,
    ) -> Result<specs::Entity, crate::Error> {
        let mut properties = Properties::new(self.ident.clone());

        for a in self.initialize.iter() {
            if let Action::With(name, value) = a {
                properties.add(name, value.clone());
            }
        }

        Ok(lazy_builder
            .with(properties)
            .with(self.ident.clone())
            .with(self.clone())
            .build())
    }
}
