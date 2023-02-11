use specs::{Entities, SystemData};
use specs::prelude::*;

use crate::BlockProperties;
use crate::state::Provider;

use super::root::RootStorageLayout;
use super::{Block, ExtensionTable, Tag};

/// Compiled system data,
///
#[derive(SystemData)]
pub struct Compiled<'a> {
    /// Entities storage,
    ///
    entities: Entities<'a>,
    /// Lazy update resource,
    ///
    lazy_update: Read<'a, LazyUpdate>,
    /// Block storage,
    ///
    blocks: ReadStorage<'a, Block>,
    /// Extension tables,
    ///
    extensions: ReadStorage<'a, ExtensionTable>,
    /// Block property storage,
    ///
    properties: ReadStorage<'a, BlockProperties>,
    /// Tag storage,
    /// 
    tags: ReadStorage<'a, Tag>,
}

impl<'a> Provider<'a, RootStorageLayout<'a>> for Compiled<'a> {
    fn provide(&'a self) -> RootStorageLayout<'a> {
        (&self.blocks, &self.extensions, &self.properties, self.tags.maybe())
    }
}

impl<'a> AsRef<Entities<'a>> for Compiled<'a> {
    fn as_ref(&self) -> &Entities<'a> {
        &self.entities
    }
}

