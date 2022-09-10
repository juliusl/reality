use super::AttributeParser;

/// Trait to implement for custom special attributes
/// 
pub trait SpecialAttribute {
    /// Ident for the attribute, 
    /// 
    /// Should be parsable by Elements::Identifier. When the 
    /// identifier is encountered, it will call Self::parse(..)
    /// 
    fn ident() -> &'static str;

    /// Returns a stack of attributes parsed from content,
    /// 
    /// Content will include everything after the attribute type identifier
    /// 
    fn parse(parser: &mut AttributeParser, content: String);
}

/// Struct for passing types that implement SpecialAttribute
/// 
#[derive(Clone)]
pub struct CustomAttribute(
    /// Identifier
    &'static str, 
    /// Parse function
    fn(&'_ mut AttributeParser, String)
);

impl CustomAttribute {
    /// Returns a new struct from a special attribute type
    /// 
    pub fn new<S>() -> Self 
    where 
        S: SpecialAttribute {
            Self(S::ident(), S::parse)
    }

    /// Returns the ident,
    /// 
    pub fn ident(&self) -> String {
        self.0.to_string()
    }

    /// Returns the parser function,
    /// 
    pub fn parse(&self, parser: &mut AttributeParser, content: impl AsRef<str>) {
        (self.1)(parser, content.as_ref().trim().to_string())
    }
}

impl<T> From<T> for CustomAttribute 
where
    T: SpecialAttribute
{
    fn from(_: T) -> Self {
        CustomAttribute(T::ident(), T::parse)
    }
}
