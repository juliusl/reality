use std::ops::Deref;

use specs::{World, LazyUpdate, WorldExt};

use crate::{v2::Build, Identifier, BlockProperties};

use super::PacketHandler;

/// Struct to build a world from interop packets,
/// 
pub struct WorldBuilder {
    /// World being built,
    /// 
    world: World,
}

impl WorldBuilder {
    /// Returns a new world builder,
    /// 
    pub fn new() -> Self {
        let mut world = World::new();
        world.register::<Identifier>();
        world.register::<BlockProperties>();
        WorldBuilder { world  }
    }
}

impl PacketHandler for WorldBuilder {
    fn on_packet(&mut self, packet: super::Packet) -> Result<(), crate::Error> {
        let lzb = self.world.fetch::<LazyUpdate>();
        let lzb = lzb.deref().create_entity(&self.world.entities());
        packet.build(lzb).ok();
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