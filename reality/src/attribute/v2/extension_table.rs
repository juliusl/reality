use std::sync::Arc;
use std::collections::HashMap;
use specs::VecStorage;
use specs::Component;
use crate::wire::Interner;

use super::action::extensions::Extend;
use super::action::extensions::Build;
use super::action::extensions::BuildRoot;

/// An extension table is a component that maps to extension implementations,
///
#[derive(Default, Component, Clone)]
#[storage(VecStorage)]
pub struct ExtensionTable {
    /// Interner for mapping identifiers to keys,
    ///
    interner: Interner,
    /// Hash map collection of extend actions,
    ///
    extend: HashMap<u64, Arc<dyn Extend>>,
    /// Hash map collection of build actions,
    /// 
    build: HashMap<u64, Arc<dyn Build>>,
    /// Hash map collection of build root actions,
    /// 
    build_root: HashMap<u64, Arc<dyn BuildRoot>>,
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
    pub fn extend(&self, ident: impl AsRef<str>) -> Option<Arc<dyn Extend>> {
        self.extend
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

    /// Adds an expand action to the extension table,
    ///
    pub fn add_expand<A: Ident + Extend>(&mut self, action: A) {
        let action = Arc::new(action);
        let key = self.key(action.clone());

        self.extend.insert(key, action.clone());
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

    /// Returns the key for this action updating the interner,
    ///
    #[inline]
    fn key(&mut self, action: Arc<impl Ident>) -> u64 {
        let key = action.clone().ident();
        self.interner.add_ident(key)
    }
}
