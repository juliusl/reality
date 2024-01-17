use crate::AttributeParser;
use crate::Shared;

/// Trait for "Host" types,
///
/// A Host Type maintains a braoder scope and broader lifecycle.
///
pub trait RegisterWith {
    /// Registers a parser plugin to use when compiling workspaces,
    ///
    fn register_with(&mut self, plugin: fn(&mut AttributeParser<Shared>));
}
