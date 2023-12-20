use std::str::FromStr;

use crate::prelude::ReprFactory;
use crate::prelude::FieldLevel;
use crate::interner::InternerFactory;

/// Trait allowing a type to identify one of it's fields by offset,
///
pub trait Field<const OFFSET: usize>: Send + Sync + 'static {
    /// Associated type that implements FromStr and is the resulting type
    /// when a field has been parsed for this field,
    ///
    type ParseType: FromStr + Send + Sync + 'static;

    /// Associated type that is projected by the implementing type for this field,
    ///
    /// **TODO**: By default this can be the same as the parse type. If associated type defaults
    /// existed, then the default would just be the ParseType.
    ///
    type ProjectedType: Send + Sync + 'static;

    /// Name of the field,
    ///
    fn field_name() -> &'static str;

    /// Creates and returns a representation factory at repr level 1,
    /// 
    fn create_repr<I: InternerFactory + Default>() -> anyhow::Result<ReprFactory<I>>
    where
        Self: Sized,
    {
        let mut factory = ReprFactory::<I>::describe_resource::<Self::ProjectedType>();

        factory.push_level(FieldLevel::new::<OFFSET, Self>())?;

        Ok(factory)
    }
}
