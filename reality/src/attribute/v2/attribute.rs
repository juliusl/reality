use specs::Entity;

use crate::Value;

use super::Action;

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
    actions: Vec<Action>,
}
