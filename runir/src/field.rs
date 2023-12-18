use std::str::FromStr;

/// Trait allowing a type to identify one of it's fields by offset,
///
pub trait Field<const OFFSET: usize> {
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
}
