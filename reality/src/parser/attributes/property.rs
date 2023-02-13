use crate::{Value, Attributes, AttributeParser};

/// Wrapper struct over property attribute handler function,
/// 
#[derive(Clone)]
pub struct PropertyAttribute(
    /// Handler,
    pub(crate) fn(&AttributeParser, Attributes),
);

impl PropertyAttribute {
    /// Called on a property attribute,
    /// 
    pub fn on_property_attribute(&self, parser: &AttributeParser, property_type: Attributes) {
        self.0(parser, property_type);
    }
}

