use specs::join::MaybeJoin;
use specs::ReadStorage;
use toml_edit::table;
use toml_edit::Document;
use toml_edit::Item;

use crate::BlockProperties;
use crate::state::Loader;

use super::{Block, ExtensionTable, Tag, ValueProvider};

/// Storage layout of root components,
/// 
pub type RootStorageLayout<'a> = (
    &'a ReadStorage<'a, Block>,
    &'a ReadStorage<'a, ExtensionTable>,
    &'a ReadStorage<'a, BlockProperties>,
    MaybeJoin<&'a ReadStorage<'a, Tag>>,
);

/// Struct containing root entity data,
///
pub struct Root<'a> {
    /// Block this root belongs to,
    ///
    block: &'a Block,
    /// Extensions registered w/ this root,
    ///
    extensions: &'a ExtensionTable,
    /// Properties defined in the block,
    ///
    properties: &'a BlockProperties,
    /// Optional tag component,
    ///
    tag: Option<&'a Tag>,
}

impl<'a> Loader for Root<'a> {
    type Layout = RootStorageLayout<'a>;

    fn load((block, extensions, properties, tag): <Self::Layout as specs::Join>::Type) -> Self {
        Self { block, extensions, properties, tag }
    }
}

impl<'a> Root<'a> {
    /// Returns the extension table,
    ///
    pub fn extensions(&'a self) -> &'a ExtensionTable {
        self.extensions
    }

    /// Copies properties from this root to a document,
    ///
    pub fn copy_to(&self, document: &mut Document) {
        let block_name = self.block.name().unwrap_or_default();
        let block_symbol = self.block.symbol().unwrap_or_default();
        let root_name = self.properties.name();

        let mut _table = table();
        _table.as_table_mut().map(|t| {
            for (name, property) in self.properties.iter_properties() {
                let rvalue = table();

                match property {
                    crate::BlockProperty::Single(prop) => {}
                    crate::BlockProperty::List(props) => {}
                    _ => {}
                }

                t[name] = rvalue;
            }
        });

        if let Some(Tag(tag)) = self.tag.as_ref() {
            document[&block_name][&block_symbol][root_name][tag] = _table;
        } else {
            document[&block_name][&block_symbol][root_name] = _table;
        }
    }
}

impl<'a> core::ops::Index<&'a str> for Root<'a> {
    type Output = Item;

    fn index(&self, index: &'a str) -> &Self::Output {
        let _index = &self.block[self.properties.name()];
        if let Some(Tag(tag)) = self.tag {
            &_index[tag][index]
        } else {
            &_index[index]
        }
    }
}

impl<'a> ValueProvider<'a> for Root<'a> {}
