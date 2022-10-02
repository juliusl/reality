use atlier::system::Value;

use super::SpecialAttribute;

/// Simple custom attribute that defines a bool property named `cache`
/// 
pub struct Cache();

impl SpecialAttribute for Cache {
    fn ident() -> &'static str {
        "cache"
    }

    fn parse(parser: &mut super::AttributeParser, _: impl AsRef<str>) {
        parser.define("cache", Value::Bool(true));
    }
}