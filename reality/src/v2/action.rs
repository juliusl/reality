use specs::Component;
use specs::Entity;
use specs::VecStorage;
use tracing::trace;
use tracing::warn;

use crate::Error;
use crate::Identifier;
use crate::Value;

use super::DispatchSignature;
use super::GetMatches;
use super::Properties;
use super::Property;
use super::Visitor;

/// Enumeration of attribute actions that apply during the transient phase of the attribute's lifecycle,
///
#[derive(Clone, Debug)]
pub enum Action {
    /// Applies a name/value,
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
    /// Sets a property,
    ///
    Set(String, Property),
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

/// Returns a set action,
///
pub fn set(name: impl Into<String>, property: impl Into<Property>) -> Action {
    Action::Set(name.into(), property.into())
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
    /// Returns an action that will apply a property,
    ///
    pub fn with(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.actions.push(with(name, value));
        self
    }

    /// Returns an extend action,
    ///
    pub fn extend(mut self, ident: impl Into<Identifier>) -> Self {
        let ident = ident.into();
        self.actions.push(extend(&ident));
        self
    }

    /// Returns a doc comment action,
    ///
    pub fn doc(mut self, comment: impl Into<String>) -> Self {
        self.actions.push(doc(comment));
        self
    }

    /// Returns a set action,
    ///
    pub fn set(mut self, name: impl Into<String>, property: impl Into<Property>) -> Self {
        self.actions.push(set(name, property));
        self
    }

    /// Apply actions to properties,
    /// 
    // pub fn apply(&self, properties: &mut Properties) {
    //     for a in self.actions.iter() {
    //         match a {
    //             Action::With(name, value) => {
    //                 properties.add(name, value.clone());
    //             },
    //             Action::Set(name, prop) => {
    //                 properties.set(name, prop.clone());
    //             },
    //             Action::Extend(_) => {
    //                 continue;
    //             },
    //             Action::Config(_, _) => {
    //                 continue;
    //             },
    //             Action::Doc(_) => {
                    
    //             },
    //         }
    //     }
    // }
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

    pub fn config(&self, _: Entity, target: &mut impl Visitor) -> Result<(), Error> {
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
                                DispatchSignature::ConfigRootExtProperty {
                                    config,
                                    name,
                                    ext,
                                    extname,
                                    property,
                                } => {
                                    trace!(
                                        config,
                                        name,
                                        ext,
                                        extname,
                                        property,
                                        "Detected Config Root Extension Property --"
                                    );
                                    target.visit_property(property, prop);
                                    target.visit_extension(ident);
                                }
                                DispatchSignature::ExtendedProperty {
                                    config,
                                    name,
                                    extension,
                                    property: Some(property),
                                } => {
                                    trace!(
                                        config,
                                        name,
                                        extension,
                                        property,
                                        "Detected Extended Property Signature --"
                                    );
                                    if let Some(properties) = prop.as_properties() {
                                        for (name, prop) in properties
                                            .iter_properties()
                                            .filter(|(name, _)| *name != property)
                                        {
                                            let config_ext = format!("{config}.{extension}.{name}")
                                                .parse::<Identifier>()?;
                                            target.visit_property(name, prop);
                                            target.visit_extension(&config_ext);
                                        }

                                        let config_prop = format!("{name}.{extension}.{property}")
                                            .parse::<Identifier>()?;
                                        let prop = properties.property(property).expect("should be a property since this is an extended property");
                                        target.visit_property(property, prop);
                                        target.visit_extension(&config_prop);
                                    }
                                }
                                DispatchSignature::ExtendedProperty {
                                    config,
                                    name,
                                    extension,
                                    property: None,
                                } => {
                                    trace!(
                                        config,
                                        name,
                                        extension,
                                        "Detected Extended Property Signature --"
                                    );
                                    if let Some(properties) = prop.as_properties() {
                                        for (name, prop) in properties.iter_properties() {
                                            let config_ext = format!("{config}.{extension}.{name}")
                                                .parse::<Identifier>()?;
                                            target.visit_property(name, prop);
                                            target.visit_extension(&config_ext);
                                        }
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
