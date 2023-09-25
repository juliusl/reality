use std::fmt::Debug;

use super::StorageTarget;
use super::AttributeParser;

/// Trait to implement a type as an AttributeType,
/// 
pub trait AttributeType<S: StorageTarget> {
    /// Identifier of the attribute type,
    /// 
    /// This identifier will be used by runmd to load this type
    /// 
    fn ident() -> &'static str;

    /// Parse content received by the runmd node into state w/ an attribute parser,
    ///
    /// The attribute parser will be given access to the storage target for the block this
    /// attribute declaration belongs to.
    /// 
    fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>);
}

/// Struct for a concrete conversion of a type that implements AttributeType,
/// 
/// Allows the parse function to be stored and recalled
/// 
pub struct AttributeTypeParser<S: StorageTarget>(
    /// Identifier
    String,
    /// Parse function
    fn(&mut AttributeParser<S>, String),
);

impl<S: StorageTarget> AttributeTypeParser<S> {
    /// Creates a new parser 
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

    /// Returns a reference to this ident,
    ///
    pub fn ident(&self) -> &str {
        self.0.as_str()
    }

    /// Executes the stored parse function,
    /// 
    pub fn parse(&self, parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
        (self.1)(parser, content.as_ref().trim().to_string())
    }
}

impl<S: StorageTarget> Clone for AttributeTypeParser<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<S: StorageTarget> Debug for AttributeTypeParser<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AttributeTypeParser").field(&self.0).finish()
    }
}
