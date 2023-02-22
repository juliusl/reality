use std::collections::HashMap;
use std::ops::Deref;
use specs::WorldExt;
use specs::World;
use specs::LazyUpdate;
use specs::HashMapStorage;
use specs::Entity;
use specs::Component;
use specs::Builder;
use tracing::trace;
use crate::Identifier;
use crate::BlockProperties;
use crate::v2::Build;
use crate::v2::BlockList;
use crate::v2::Block;
use crate::v2::Attribute;
use super::PacketHandler;

/// Struct to build a world from interop packets,
///
pub struct WorldBuilder {
    /// World being built,
    ///
    world: World,
    /// Block list,
    ///
    block_list: BlockList,
    /// Build log,
    /// 
    build_log: BuildLog,
}

impl WorldBuilder {
    /// Returns a new world builder,
    ///
    pub fn new() -> Self {
        let mut world = World::new();
        world.register::<Identifier>();
        world.register::<BlockProperties>();
        world.register::<Block>();
        world.register::<Attribute>();
        world.register::<BuildLog>();
        WorldBuilder {
            world,
            block_list: BlockList::default(),
            build_log: BuildLog::default(),
        }
    }

    /// Runs a lazy build,
    ///
    pub fn lazy_build(&self, build: &impl Build) -> Result<specs::Entity, crate::Error> {
        let lzb = self.world.fetch::<LazyUpdate>();
        let lzb = lzb.deref().create_entity(&self.world.entities());

        build.build(lzb)
    }

    /// Updates the world builder,
    ///
    pub fn update(&self) -> Result<specs::Entity, crate::Error> {
        let lzb = self.world.fetch::<LazyUpdate>();
        let lzb = lzb.deref().create_entity(&self.world.entities());

        self.build(lzb)
    }

    /// Returns the current build log,
    /// 
    pub fn build_log(&self, build: Entity) -> BuildLog {
        self.world.read_component::<BuildLog>().get(build).cloned().unwrap_or(self.build_log.clone())
    }
}

impl PacketHandler for WorldBuilder {
    fn on_packet(&mut self, packet: super::Packet) -> Result<(), crate::Error> {
        self.block_list.on_packet(packet.clone())?;

        if let Some(built) = self.lazy_build(&packet).ok() {
            trace!("Built packet, {:?}", built);
            self.build_log.index.insert(packet.identifier.commit()?, built);
        }

        Ok(())
    }
}

impl AsRef<World> for WorldBuilder {
    fn as_ref(&self) -> &World {
        &self.world
    }
}

impl AsMut<World> for WorldBuilder {
    fn as_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl Build for WorldBuilder {
    fn build(
        &self,
        lazy_builder: specs::world::LazyBuilder,
    ) -> Result<specs::Entity, crate::Error> {
        let mut log = self.build_log.clone();
        for (ident, block) in self.block_list.blocks() {
            let e = self.lazy_build(block)?;
            log.index.insert(ident.commit()?, e);

            for a in block.attributes() {
                let e = self.lazy_build(a)?;
                log.index.insert(a.ident.commit()?, e);
            }
        }

        Ok(lazy_builder.with(log).build())
    }
}

/// Log of built entities,
/// 
#[derive(Component, Clone, Default)]
#[storage(HashMapStorage)]
pub struct BuildLog {
    index: HashMap<Identifier, Entity>,
}

impl BuildLog {
    pub fn index(&self) -> &HashMap<Identifier, Entity> {
        &self.index
    }
}