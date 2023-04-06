use specs::Component;
use specs::HashMapStorage;
use specs::VecStorage;

use crate::Error;
use crate::Identifier;
use crate::Value;

use super::Config;
use super::Property;

/// Enumeration of attribute actions that apply during the transient phase of the attribute's lifecycle,
///
#[derive(Clone, Debug)]
pub enum Action {
    /// Applies a value to the current namespace,
    ///
    With(String, Value),
    /// Extends the current namespace,
    ///
    Extend(Identifier),
    /// Configures a property,
    ///
    Config(Identifier, Property),
    /// Doc comment,
    ///
    Doc(String),
}

/// Returns an action that will apply a property,
///
pub fn with(name: impl Into<String>, value: impl Into<Value>) -> Action {
    Action::With(name.into(), value.into())
}

/// Returns an extend action,
///
pub fn extend(ident: &Identifier) -> Action {
    Action::Extend(ident.clone())
}

/// Returns a doc comment action,
///
pub fn doc(comment: impl Into<String>) -> Action {
    Action::Doc(comment.into().trim().to_string())
}

/// Returns a config action,
///
pub fn config(config_ident: &Identifier, config: &Property) -> Action {
    Action::Config(config_ident.clone(), config.clone())
}

/// Component for a list of actions to apply,
///
#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct ActionBuffer {
    /// Actions that are pending application,
    ///
    actions: Vec<Action>,
}

impl ActionBuffer {
    /// Returns a new empty action buffer,
    /// 
    pub const fn new() -> Self {
        Self { actions: vec![] }
    }
    /// Pushes a new config to the buffer,
    /// 
    pub fn push_config(&mut self, config_ident: &Identifier, _config: &Property) {
        self.actions.push(config(config_ident, _config));
    }

    /// Configures a target that implements the Config trait,
    /// 
    pub fn config(&self, target: &mut impl Config) -> Result<(), Error> {
        for action in self.actions.iter() {
            match action {
                Action::Config(ident, config) => {
                    target.config(ident, config)?;
                }
                _ => { continue; }
            }
        }

        Ok(())
    }
}
