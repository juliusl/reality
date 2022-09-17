use std::{path::PathBuf, collections::BTreeSet};

use atlier::system::Value;
use specs::{WorldExt, Component, DefaultVecStorage};
use tracing::{event, Level};

use crate::Interpreter;

use super::{custom::SpecialAttribute, AttributeParser};

/// Struct for handling local files,
/// 
/// Implements SpecialAttribute, with `.file` as the identifier,
/// 
/// # Special attribute behavior
/// 
/// Looks at path in the current local file system, then 
/// 1) adds a stable empty binary vector attribute
/// 2) maps file metadata as properties of the stable attribute
/// 3) maps a `file` complex to filter the metadata attributes
/// 
/// # Interpreter behavior
/// 
/// Looks for stable attributes with a `file` complex, interprets, and 
/// returns a FileDescriptor component
/// 
#[derive(Debug, Default, Component)]
#[storage(DefaultVecStorage)]
pub struct FileDescriptor {

}

impl SpecialAttribute for FileDescriptor {
    fn ident() -> &'static str {
        "file"
    }

    /// Parses a file path, and maps a snapshot of transient state,
    ///
    /// Does not read the contents of the file on disk, so that it can
    /// be handled by a system.
    ///
    fn parse(attr_parser: &mut AttributeParser, content: String) {
        assert!(attr_parser.symbol.is_none(), "Can only be used when adding a stable attribute");

        let name = attr_parser.name.clone().expect("has name").to_string();
        let path = PathBuf::from(content);

        // Map if the file exists
        attr_parser.define("exists", Value::Bool(path.exists()));
        
        // Map file path parts
        match path.canonicalize() {
            Ok(path) => {
                // Map the parent dir
                if let Some(parent) = path.parent() {
                    attr_parser.define("parent", Value::Symbol(
                        parent.to_str().expect("is string").to_ascii_lowercase(),
                    ));
                }

                // Map the file extension
                if let Some(extension) = path.extension() {
                    attr_parser.define("extension", Value::Symbol(
                        extension.to_str().expect("is string").to_ascii_lowercase(),
                    ));
                }

                // Map the file name
                if let Some(filename) = path.file_name() {
                    attr_parser.define("filename", Value::Symbol(
                        filename.to_str().expect("is string").to_ascii_lowercase(),
                    ));
                }
            }
            Err(err) => {
                // If the directory does not exist, 
                // then the file path cannot be canonicalized
                event!(Level::ERROR, "error {err}")
            }
        }

        attr_parser.define("file", Value::Complex(BTreeSet::from_iter(vec![
            "parent".to_string(),
            "extension".to_string(),
            "filename".to_string(),
            "exists".to_string(),
        ])));

        // Add the stable attribute w/ an empty vector
        attr_parser.set_name(name);
        attr_parser.set_value(Value::BinaryVector(vec![]));
        attr_parser.parse_attribute();
    }
}

impl Interpreter for FileDescriptor {
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