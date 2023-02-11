use crate::Value;

use super::{action, Action};

/// V2 version of the Attribute struct,
///
pub struct Attribute {
    /// Identifier string,
    ///
    pub ident: String,
    /// Value of this attribute,
    ///
    pub value: Value,
    /// Stack of actions that will be applied to this attribute during it's transient phase,
    ///
    action_stack: Vec<Action>,
}

impl Attribute {
    /// Returns an iterator over the action stack,
    /// 
    pub fn action_stack(&self) -> impl Iterator<Item = &Action> {
        self.action_stack.iter()
    }

    /// Returns self with a `with` action,
    /// 
    pub fn with(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.action_stack.push(action::with(name, value));
        self
    }

    /// Returns self with a `define` action,
    /// 
    pub fn define(mut self) -> Self {
        self.action_stack.push(action::define());
        self
    }

    /// Returns self with an `expand` action,
    ///
    pub fn extend(mut self, ident: impl Into<String>) -> Self {
        self.action_stack.push(action::extend(ident));
        self
    }

    /// Returns self with a `build` action,
    ///
    pub fn build(mut self, ident: impl Into<String>) -> Self {
        self.action_stack.push(action::build(ident));
        self
    }

    /// Returns self with a `build_root` action,
    ///
    pub fn build_root(mut self, ident: impl Into<String>) -> Self {
        self.action_stack.push(action::build_root(ident));
        self
    }
}
