use crate::{AttributeTypeParser, StorageTarget};

/// Struct containing all attributes,
///
pub struct Block {}

pub trait BlockObject<Storage: StorageTarget> {
    /// Return a list of properties this block object can define,
    ///
    fn properties() -> Vec<AttributeTypeParser<Storage>>;
}

impl<Storage: StorageTarget> BlockObject<Storage> for () {
    fn properties() -> Vec<AttributeTypeParser<Storage>> {
        
        let b = "".parse::<bool>();

        vec![
            AttributeTypeParser::new_with("bool", |parser, input| {
                if let Some(storage) = parser.storage() {
                }
            })]
    }
}

impl<S: StorageTarget + 'static> crate::AttributeType<S> for Block {
    fn ident() -> &'static str {
        todo!()
    }

    fn parse(parser: &mut crate::AttributeParser<S>, content: impl AsRef<str>) {
        // Each attribute in the block can add extensions they support
        // Extensions share the same storage target
        // Attributes do not share storage targets w/ other attributes
        // Blocks can access all storage targets

        // First, parse content and if successful, editable properties

        // Then, add parsers for any properties this type can parse
        parser.with_parseable_as::<bool>("debug");
    }
}