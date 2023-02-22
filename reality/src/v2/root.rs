use specs::Builder;
use specs::Component;
use specs::VecStorage;

use super::Properties;
use super::action;
use super::Action;
use super::Build;
use crate::Elements;
use crate::Identifier;
use crate::Value;

/// V2 version of the Attribute struct,
///
#[derive(Component, Clone, Debug)]
#[storage(VecStorage)]
pub struct Root {
    /// Identifier,
    ///
    pub ident: Identifier,
    /// Value of this root,
    ///
    pub value: Value,
    /// Stack of actions that will be applied to this attribute during it's transient phase,
    ///
    action_stack: Vec<Action>,
}

impl Root {
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
                }
                _ => {
                    continue;
                }
            }
        }
    }

    /// Returns an iterator over the extensions required by this attribute,
    ///
    pub fn extensions(&self) -> impl Iterator<Item = &Identifier> {
        self.action_stack().filter_map(|a| match a {
            Action::Extend(ident) => Some(ident),
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
    pub fn extend(mut self, ident: &Identifier) -> Self {
        self.action_stack.push(action::extend(ident));
        self
    }
}

impl Build for Root {
    fn build(
        &self,
        lazy_builder: specs::world::LazyBuilder,
    ) -> Result<specs::Entity, crate::Error> {
        let mut properties = Properties::new(self.ident.to_string());

        for a in self.action_stack.iter() {
            if let Action::With(name, value) = a {
                properties.add(name, value.clone());
            }
        }

        Ok(lazy_builder
            .with(properties)
            .with(self.clone())
            .with(self.ident.commit()?)
            .build())
    }
}
