use std::collections::BTreeSet;

use atlier::system::Value;
use specs::{Component, DefaultVecStorage, WorldExt};

use crate::Interpreter;

use super::{custom::SpecialAttribute, AttributeParser};

/// Struct for handling blobs,
///
/// Implements SpecialAttribute, with `.blob` as the identifier,
///
/// # Special attribute behavior
///
/// Looks at path in the current local file system, then
/// 1) adds a stable empty binary vector attribute
/// 2) maps the address to a property
///
/// # Interpreter behavior
///
/// Looks for stable attributes with a `blob` complex, interprets, and
/// returns a BlobDescriptor component
///
#[derive(Debug, Default, Component)]
#[storage(DefaultVecStorage)]
pub struct BlobDescriptor {}

impl SpecialAttribute for BlobDescriptor {
    fn ident() -> &'static str {
        "blob"
    }

    /// Interprets the content as an address and maps a snapshot of transient state,
    ///
    /// Does not read the contents of the file on disk, so that it can
    /// be handled by a system.
    ///
    fn parse(attr_parser: &mut AttributeParser, content: String) {
        let name = attr_parser.name.clone().expect("An identifier must exist");

        // Map the blob address to an attribute
        attr_parser.define("address", 
            Value::Symbol(content.to_ascii_lowercase())
        );

        attr_parser.define("blob", 
            Value::Complex(BTreeSet::from_iter(vec![
            "address".to_string(),
        ])));

        // Add the stable attribute w/ an empty vector
        attr_parser.add(name, Value::BinaryVector(vec![]));
    }
}

impl Interpreter for BlobDescriptor {
    type Output = Self;

    fn initialize(&self, world: &mut specs::World) {
        world.register::<Self>();
    }

    fn interpret(&self, _block: &crate::Block) -> Option<Self::Output> {
        todo!()
    }

    fn interpret_mut(&mut self, _block: &crate::Block) {
        todo!()
    }
}
