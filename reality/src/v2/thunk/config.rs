use crate::{v2::Property, Identifier, Error};

/// Implement to configure w/ identifier & property,
/// 
pub trait Config {
    /// Configures self w/ an identifier and property,
    /// 
    fn config(&mut self, ident: &Identifier, property: &Property) -> Result<(), Error>;
}

impl<T> Config for T 
where
    for<'a> T: From<&'a Property>,
{
    fn config(&mut self, _: &Identifier, property: &Property) -> Result<(), Error> {
        *self = property.into();

        Ok(())
    }
}

/// Implement to apply a rule to a property,
/// 
pub trait Apply {
    /// Applies rule w/ rule_name to property and returns the result,
    /// 
    fn apply(&self, rule_name: impl AsRef<str>, property: &Property) -> Result<Property, Error>;
}

impl Apply for () {
    fn apply(&self, _: impl AsRef<str>, property: &Property) -> Result<Property, Error> {
        Ok(property.clone())
    }
}