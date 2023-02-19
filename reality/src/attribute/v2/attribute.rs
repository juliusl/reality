use specs::VecStorage;
use specs::Component;

use crate::Elements;
use crate::Value;
use super::Identifier;
use super::Action;
use super::action;

/// V2 version of the Attribute struct,
///
#[derive(Component, Clone, Debug)]
#[storage(VecStorage)]
pub struct Attribute {
    /// Identifier,
    ///
    pub ident: Identifier,
    /// Value of this attribute,
    ///
    pub value: Value,
    /// Stack of actions that will be applied to this attribute during it's transient phase,
    ///
    action_stack: Vec<Action>,
}

impl Attribute {
    /// Returns a new attribute,
    ///
    pub fn new(ident: Identifier, value: impl Into<Value>) -> Self {
        Self {
            ident,
            value: value.into(),
            action_stack: vec![],
        }
    }

    /// Includes a tag w/ the identifier,
    /// 
    pub fn set_tags(&mut self, tags: impl AsRef<str>) {
        use logos::Logos;

        let mut elements = Elements::lexer(tags.as_ref());

        while let Some(element) = elements.next() {
            match element {
                Elements::Identifier(tag) => {
                    for t in tag.split(":") {
                        self.ident.add_tag(t);
                    }
                    return;
                },
                _ => {
                    continue;
                }
            }
        }
    }

    /// Returns an iterator over the extensions required by this attribute,
    ///
    pub fn requires(&self) -> impl Iterator<Item = &Action> {
        self.action_stack().filter_map(|a| match a {
            Action::Extend(_, _) => Some(a),
            _ => None,
        })
    }

    /// Returns an iterator over the action stack,
    ///
    pub fn action_stack(&self) -> impl Iterator<Item = &Action> {
        self.action_stack.iter()
    }

    /// Pushes an action on the stack,
    ///
    pub fn push(&mut self, action: Action) {
        self.action_stack.push(action);
    }

    /// Returns self with a `with` action,
    ///
    pub fn with(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.action_stack.push(action::with(name, value));
        self
    }

    /// Returns self with an `expand` action,
    ///
    pub fn extend(mut self, ident: impl Into<String>, value: impl Into<Value>) -> Self {
        self.action_stack.push(action::extend(ident, value));
        self
    }
}
