
use crate::Value;

mod extend;
mod build;
mod build_root;

/// Types of extension actions that can be applied on built attributes,
/// 
pub mod extensions {
    pub use super::extend::Extend;
    pub use super::build::Build;
    pub use super::build_root::BuildRoot;
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
    /// Extend is an extension action that will expand into a vector of actions when applied,
    ///
    Extend(String),
    /// Build is an extension action that will build an entity,
    ///
    Build(String),
    /// Build root is an extension action that will build an entity from a root,
    /// 
    BuildRoot(String),
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

/// Returns an extend action,
///
pub fn extend(ident: impl Into<String>) -> Action {
    Action::Extend(ident.into())
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

