/// Wrapper struct for a tag value,
///
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Tag<'a>(pub &'a str);