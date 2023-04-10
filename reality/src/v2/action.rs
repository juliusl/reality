use specs::Component;
use specs::VecStorage;
use tracing::trace;
use tracing::warn;

use crate::Error;
use crate::Identifier;
use crate::Value;

use super::Config;
use super::DispatchSignature;
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
                Action::Config(ident, prop) => {
                    let sigs = DispatchSignature::get_match(ident);
                    if sigs.len() > 1 {
                        warn!(
                            "Multiple signatures detected, {} {:#} -- {:?}",
                            ident, ident, sigs
                        );
                    }

                    match sigs.first() {
                        Some(sig) => {
                            match sig {
                                DispatchSignature::ExtendedProperty {
                                    config,
                                    name,
                                    extension,
                                    property,
                                } => {
                                    trace!(config, name, extension, property, "Detected Extended Property Signature --");
                                    if let Some(properties) = prop.as_properties() {
                                        let config_ext = format!("{config}.{extension}")
                                            .parse::<Identifier>()?;

                                        let config_prop = format!("{name}.{extension}.{property}")
                                             .parse::<Identifier>()?;

                                        for (name, prop) in properties
                                            .iter_properties()
                                            .filter(|(name, _)| *name != property)
                                        {
                                            let ident = config_ext.branch(name)?;
                                            trace!(
                                                "Extension config -- {:<10} {:<10} {:?}",
                                                ident.root(),
                                                ident,
                                                prop
                                            );
                                            // Apply extended config to the extension config
                                            target.config(&ident, prop)?;
                                        }

                                        let prop = properties.property(property).expect("should be a property since this is an extended property");
                                        // Apply config to the primary property
                                        trace!("Config           -- {:<10} {:<20} {:?>4}", config_prop.root(), config_prop, prop);
                                        target.config(&config_prop, prop)?;

                                        trace!("Configured Extended Property -- \n");
                                    }
                                }
                                _ => {}
                            }
                        }
                        None => {
                            continue;
                        }
                    }
                }
                _ => {
                    continue;
                }
            }
        }

        Ok(())
    }

    /// Iterate over stored actions,
    ///
    pub fn iter_actions(&self) -> impl Iterator<Item = &Action> {
        self.actions.iter()
    }
}
