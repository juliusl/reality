
use crate::Value;

mod build;

/// Types of extension actions that can be applied on built attributes,
/// 
pub mod extensions {
    pub use super::build::Build;
}

/// Enumeration of attribute actions that apply during the transient phase of the attribute's lifecycle,
///
#[derive(Clone, Debug)]
pub enum Action {
    /// Applies a value to the current namespace,
    /// 
    With(String, Value),
    /// Extends the current namespace,
    /// 
    Extend(String, Value),
}

/// Returns an action that will apply a property,
///
pub fn with(name: impl Into<String>, value: impl Into<Value>) -> Action {
    Action::With(name.into(), value.into())
}

/// Returns an extend action,
///
pub fn extend(ident: impl Into<String>, value: impl Into<Value>) -> Action {
    Action::Extend(ident.into(), value.into())
}
