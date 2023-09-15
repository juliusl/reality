use std::fmt::Debug;

use super::StorageTarget;
use super::AttributeParser;

/// Trait to implement for custom special attributes
///
pub trait AttributeType<S: StorageTarget> {
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
    fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>);
}

/// Struct for passing types that implement SpecialAttribute
///
pub struct CustomAttribute<S: StorageTarget>(
    /// Identifier
    String,
    /// Parse function
    fn(&mut AttributeParser<S>, String),
);

impl<S: StorageTarget> CustomAttribute<S> {
    /// Returns a new struct from a special attribute type
    ///
    pub fn new<A>() -> Self
    where
        A: AttributeType<S>,
    {
        Self(A::ident().to_string(), A::parse)
    }

    pub fn new_with(ident: impl AsRef<str>, parse: fn(&mut AttributeParser<S>, String)) -> Self {
        Self(ident.as_ref().to_string(), parse)
    }

    /// Returns the ident,
    ///
    pub fn ident(&self) -> String {
        self.0.to_string()
    }

    /// Returns the parser function,
    ///
    pub fn parse(&self, parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
        (self.1)(parser, content.as_ref().trim().to_string())
    }
}

impl<S: StorageTarget> Clone for CustomAttribute<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<S: StorageTarget> Debug for CustomAttribute<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CustomAttribute").field(&self.0).finish()
    }
}
