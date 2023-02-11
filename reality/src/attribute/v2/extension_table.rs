use std::sync::Arc;
use std::collections::HashMap;
use specs::VecStorage;
use specs::Component;
use crate::wire::Interner;

use super::action::extensions::Build;
use super::action::extensions::BuildRoot;
use super::action::extensions::EditToml;
use super::action::extensions::Expand;
use super::action::extensions::BuildToml;

/// An extension table is a component that maps to extension implementations,
///
#[derive(Default, Component, Clone)]
#[storage(VecStorage)]
pub struct ExtensionTable {
    /// Interner for mapping identifiers to keys,
    ///
    interner: Interner,
    /// Hash map collection of expand actions,
    ///
    expands: HashMap<u64, Arc<dyn Expand>>,
    /// Hash map collection of build actions,
    /// 
    build: HashMap<u64, Arc<dyn Build>>,
    /// Hash map collection of build root actions,
    /// 
    build_root: HashMap<u64, Arc<dyn BuildRoot>>,
    /// Hash map collection of edit toml actions,
    /// 
    edit_toml: HashMap<u64, Arc<dyn EditToml>>,
    /// Hash map collection of build toml actions,
    ///
    build_toml: HashMap<u64, Arc<dyn BuildToml>>,
}

/// Trait to provide an identifier,
///
pub trait Ident
where
    Self: Send + Sync,
{
    /// Returns the identifier for this object,
    ///
    fn ident(self: Arc<Self>) -> String;
}

impl ExtensionTable {
    /// Returns a registered expand operation,
    ///
    pub fn expand(&self, ident: impl AsRef<str>) -> Option<Arc<dyn Expand>> {
        self.expands
            .get(&self.interner.ident(ident))
            .map(|e| e.clone())
    }

    /// Returns a registered build operation,
    ///
    pub fn build(&self, ident: impl AsRef<str>) -> Option<Arc<dyn Build>> {
        self.build
            .get(&self.interner.ident(ident))
            .map(|e| e.clone())
    }

    /// Returns a registered build root operation,
    ///
    pub fn build_root(&self, ident: impl AsRef<str>) -> Option<Arc<dyn BuildRoot>> {
        self.build_root
            .get(&self.interner.ident(ident))
            .map(|e| e.clone())
    }

    /// Returns a registered edit toml action,
    /// 
    pub fn edit_toml(&self, ident: impl AsRef<str>) -> Option<Arc<dyn EditToml>> {
        self.edit_toml
            .get(&self.interner.ident(ident))
            .map(|e| e.clone())
    }

    /// Returns a registered build toml action,
    /// 
    pub fn build_toml(&self, ident: impl AsRef<str>) -> Option<Arc<dyn BuildToml>> {
        self.build_toml
            .get(&self.interner.ident(ident))
            .map(|e| e.clone())
    }

    /// Adds an expand action to the extension table,
    ///
    pub fn add_expand<A: Ident + Expand>(&mut self, action: A) {
        let action = Arc::new(action);
        let key = self.key(action.clone());

        self.expands.insert(key, action.clone());
    }

    /// Adds an build action to the extension table,
    ///
    pub fn add_build<A: Ident + Build>(&mut self, action: A) {
        let action = Arc::new(action);
        let key = self.key(action.clone());

        self.build.insert(key, action.clone());
    }

    /// Adds an build root action to the extension table,
    ///
    pub fn add_build_root<A: Ident + BuildRoot>(&mut self, action: A) {
        let action = Arc::new(action);
        let key = self.key(action.clone());

        self.build_root.insert(key, action.clone());
    }

    /// Adds an edit toml action to the extension table,
    /// 
    pub fn add_edit_toml<A: Ident + EditToml>(&mut self, action: A) {
        let action = Arc::new(action);
        let key = self.key(action.clone());

        self.edit_toml.insert(key, action.clone());
    }

    /// Adds a build toml action to the extension table,
    ///
    pub fn add_build_toml<A: Ident + BuildToml>(&mut self, action: A) {
        let action = Arc::new(action);
        let key = self.key(action.clone());

        self.build_toml.insert(key, action.clone());
    }

    /// Returns the key for this action updating the interner,
    ///
    #[inline]
    fn key(&mut self, action: Arc<impl Ident>) -> u64 {
        let key = action.clone().ident();
        self.interner.add_ident(key)
    }
}
