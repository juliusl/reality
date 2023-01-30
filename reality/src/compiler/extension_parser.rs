use specs::Entity;

use crate::{AttributeParser, Value, Parser};

/// This trait defines higher level api's for common property parsing patterns,
///
/// Also built in support for handling list-type properties,
///
pub trait ExtensionParser
where
    Self: AsMut<AttributeParser>,
{
    /// Adds a parser to parse custom attribute type w/ name ident,
    /// adding a property with the ident from Self::property_definition and parses content for a number type,
    /// If a number type is present adds that value as the property value.
    ///
    fn parse_number(&mut self, ident: &'static str) {
        self.as_mut().add_custom_with(ident, |p, c| {
            if let Some((entity, ident)) = Self::propery_definition(p) {
                let mut parser = AttributeParser::default();
                let parser = parser.parse(c);

                if let Some(value) = parser.value().number() {
                    p.define_child(entity, ident, value);
                }
            }
        });
    }
    
    /// Adds a parser to parse custom attribute type w/ name ident,
    /// adding a property with the ident from Self::property_definition and the content being parsed
    ///
    fn parse_symbol(&mut self, ident: &'static str) {
        self.as_mut().add_custom_with(ident, |p, c| {
            if let Some((entity, ident)) = Self::propery_definition(p) {
                p.define_child(entity, ident, Value::Symbol(c));
            }
        });
    }

    /// Adds a parser to parse custom attribute type w/ name ident,
    /// adding a property with the ident from Self::property_definition and the value true
    ///
    fn parse_bool(&mut self, ident: &'static str) {
        self.as_mut().add_custom_with(ident, |p, _| {
            if let Some((entity, ident)) = Self::propery_definition(p) {
                p.define_child(entity, ident, true);
            }
        });
    }

    /// Returns essential values to define a property w/ the current parser,
    ///
    fn propery_definition(parser: &mut AttributeParser) -> Option<(Entity, String)> {
        if let (Some(entity), Some(ident)) =
            (parser.last_child_entity(), parser.attr_ident().cloned())
        {
            if let Some(var_name) = parser.symbol() {
                let var_name = var_name.to_string();
                parser.define_child(entity, ident, Value::Symbol(var_name.to_string()));
                Some((entity, var_name))
            } else {
                Some((entity, ident))
            }
        } else {
            None
        }
    }
}

impl ExtensionParser for AttributeParser {}
impl ExtensionParser for Parser {}
