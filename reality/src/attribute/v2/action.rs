
use crate::Value;

mod expand;
mod build;
mod build_root;
mod edit_toml;
mod build_toml;

/// Types of extension actions that can be applied on built attributes,
/// 
pub mod extensions {
    pub use super::expand::Expand;
    pub use super::build::Build;
    pub use super::build_root::BuildRoot;
    pub use super::edit_toml::EditToml;
    pub use super::build_toml::BuildToml;
}

/// Enumeration of attribute actions that apply during the transient phase of the attribute's lifecycle,
///
#[derive(Default)]
pub enum Action {
    /// This action will define a property value on the attribute's entity using the current state,
    ///
    #[default]
    Define,
    /// This action will define a property value on the attribute's entity,
    ///
    With(String, Value),
    /// Expand is an extension action that will expand into a vector of actions when applied,
    ///
    Expand(String),
    /// Build is an extension action that will build an entity,
    ///
    Build(String),
    /// Build root is an extension action that will build an entity from a root,
    /// 
    BuildRoot(String),
    /// Edit toml is an extension action that will the toml document in the current scope,
    ///
    EditToml(String),
    /// Build toml is an extension action that will build an entity using a toml document,
    ///
    BuildToml(String),
}

/// Returns an action that will apply a property,
///
pub fn with(name: impl Into<String>, value: impl Into<Value>) -> Action {
    Action::With(name.into(), value.into())
}

/// Returns an action that will apply an attribute as a property,
///
pub fn define() -> Action {
    Action::Define
}

/// Returns an expand action,
///
pub fn expand(ident: impl Into<String>) -> Action {
    Action::Expand(ident.into())
}

/// Returns a build action,
/// 
pub fn build(ident: impl Into<String>) -> Action {
    Action::Build(ident.into())
}

/// Returns a build root action,
///
pub fn build_root(ident: impl Into<String>) -> Action {
    Action::BuildRoot(ident.into())
}

/// Returns an action that edits a toml document,
/// 
pub fn edit_toml(ident: impl Into<String>) -> Action {
    Action::EditToml(ident.into())
}
/// Returns an action that builds an entity from a document,
///
pub fn build_toml(ident: impl Into<String>) -> Action {
    Action::BuildToml(ident.into())
}
