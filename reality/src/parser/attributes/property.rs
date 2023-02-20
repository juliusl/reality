use crate::AttributeParser;

/// Wrapper struct over property attribute handler function,
/// 
#[derive(Clone)]
pub struct PropertyAttribute(
    /// Handler,
    pub(crate) fn(&mut AttributeParser),
);

impl PropertyAttribute {
    /// Called on a property attribute,
    /// 
    pub fn on_property_attribute(&self, parser: &mut AttributeParser) {
        self.0(parser);
    }
}

