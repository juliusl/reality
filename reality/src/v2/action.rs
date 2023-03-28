
use crate::Identifier;
use crate::Value;

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
