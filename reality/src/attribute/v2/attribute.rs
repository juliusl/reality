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
    /// Root entity this attribute belongs to,
    /// 
    /// If set, this entity will be used to load an extension table into scope,
    /// 
    /// If not set, then the default extension table will be used,
    /// 
    root: Option<Entity>,
    /// Stack of actions that will be applied to this attribute during it's transient phase,
    ///
    actions: Vec<Action>,
}
