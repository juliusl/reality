/// Wrapper struct for a tag value,
///
#[derive(Hash, Default, Debug, Clone, PartialEq)]
pub struct Tag<'a>(pub &'a str);
