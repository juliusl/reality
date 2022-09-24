use std::{collections::BTreeSet, path::PathBuf};
use atlier::system::Value;
use specs::{Component, DefaultVecStorage, WorldExt};
use tracing::{event, Level};

use crate::{parser::attributes::Cache, Interpreter, wire::BlobDevice, BlockProperties};

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
pub struct File {
    /// Files to include w/ this component
    files: Vec<FileDescriptor>,
}

/// Descriptor of a local file,
/// 
#[derive(Debug, Default)]
pub struct FileDescriptor { 
    /// Properties of this file
    properties: BlockProperties,
    /// Cached file data
    cache: Option<BlobDevice>,
}

impl FileDescriptor {
    pub fn new(properties: BlockProperties) -> Self {
        FileDescriptor { properties, cache: None }
    }
}

impl SpecialAttribute for File {
    fn ident() -> &'static str {
        "file"
    }

    /// Parses a file path, and maps a snapshot of transient state,
    ///
    /// Does not read the contents of the file on disk, so that it can
    /// be handled by a system.
    ///
    fn parse(attr_parser: &mut AttributeParser, content: String) {
        assert!(
            attr_parser.symbol().is_none(),
            "Can only be used when adding a stable attribute"
        );

        let name = attr_parser.name().clone().expect("has name").to_string();
        let path = PathBuf::from(content);

        // Map if the file exists
        attr_parser.define("exists", Value::Bool(path.exists()));

        // Map the parent dir
        if let Some(parent) = path.parent() {
            attr_parser.define(
                "parent",
                Value::Symbol(parent.to_str().expect("is string").to_ascii_lowercase()),
            );
        }

        // Map the file extension
        if let Some(extension) = path.extension() {
            attr_parser.define(
                "extension",
                Value::Symbol(extension.to_str().expect("is string").to_ascii_lowercase()),
            );
        }

        // Map the file name
        if let Some(filename) = path.file_name() {
            attr_parser.define(
                "filename",
                Value::Symbol(filename.to_str().expect("is string").to_ascii_lowercase()),
            );
        }

        // Map file path parts
        match path.canonicalize() {
            Ok(path) => {
                attr_parser.define(
                    "absolute_path",
                    Value::Symbol(path.to_str().expect("is string").to_ascii_lowercase()),
                );
            }
            Err(err) => {
                // If the directory does not exist,
                // then the file path cannot be canonicalized
                event!(Level::ERROR, "error {err}")
            }
        }

        attr_parser.define(
            "file",
            Value::Complex(BTreeSet::from_iter(vec![
                "absolute_path".to_string(),
                "parent".to_string(),
                "extension".to_string(),
                "filename".to_string(),
                "exists".to_string(),
                "cache".to_string(),
            ])),
        );

        attr_parser.add(name, Value::BinaryVector(vec![]));

        // Add the `.cache` custom attribute type
        attr_parser.add_custom(Cache());
    }
}

impl Interpreter for File {
    type Output = Self;

    fn initialize(&self, world: &mut specs::World) {
        world.register::<Self>();
    }

    fn interpret(&self, block: &crate::Block, _: Option<&Self::Output>) -> Option<Self::Output> {
        // These are all the attributes with a `file` complex
        let files = block
            .index()
            .iter()
            .filter_map(|i| i.as_complex("file"))
            .map(FileDescriptor::new)
            .collect();

        Some(File { files })
    }
}
