use super::prelude::*;

/// Struct containing attribute parameters,
///
/// An attribute is simply a container w/ a name and an input value,
///
#[derive(Hash, Default, Debug, Clone, PartialEq)]
pub struct Attribute<'a> {
    /// Name of this attribute,
    ///
    pub name: &'a str,
    /// The input set for this attribute,
    ///
    pub input: Option<Input<'a>>,
}
