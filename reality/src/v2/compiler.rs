use std::collections::BTreeMap;
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
use crate::state::Provider;
use crate::v2::Build;
use crate::v2::BlockList;
use crate::v2::Block;
use crate::v2::Root;
use super::Properties;
use super::ThunkBuild;
use super::ThunkCall;
use super::Visitor;
use super::parser::Packet;
use super::parser::PacketHandler;

mod compiled;
pub use compiled::Compiled;
pub use compiled::Object;

/// Struct to build a world from interop packets,
///
pub struct Compiler {
    /// World being built,
    ///
    world: World,
    /// Block list,
    ///
    block_list: BlockList,
    /// Build log,
    /// 
    build_log: BuildLog,
    /// Builds,
    /// 
    builds: Vec<Entity>,
}

impl Compiler {
    /// Returns a new world builder,
    ///
    pub fn new() -> Self {
        let mut world = World::new();
        world.register::<Identifier>();
        world.register::<Properties>();
        world.register::<Block>();
        world.register::<Root>();
        world.register::<BuildLog>();
        world.register::<ThunkBuild>();
        world.register::<ThunkCall>();
        Compiler {
            world,
            block_list: BlockList::default(),
            build_log: BuildLog::default(),
            builds: vec![],
        }
    }

    /// Runs a lazy build,
    ///
    pub fn lazy_build(&self, build: &impl Build) -> Result<specs::Entity, crate::Error> {
        let lzb = self.world.fetch::<LazyUpdate>();
        let lzb = lzb.deref().create_entity(&self.world.entities());

        build.build(lzb)
    }

    /// Compiles parsed block list and consumes internal state, inserting components into world storage,
    /// 
    /// Returns the entity of the build,
    /// 
    /// Compiler can be re-used w/o removing previous built components,
    /// 
    pub fn compile(&mut self) -> Result<specs::Entity, crate::Error> {
        let build = {
            let lzb = self.world.fetch::<LazyUpdate>();
            let lzb = lzb.deref().create_entity(&self.world.entities());
    
            self.build(lzb)?
        };

        // Clear internal state,
        self.build_log.index.clear();
        self.block_list = BlockList::default();
        self.world.maintain();
        self.builds.push(build);

        Ok(build)
    }

    /// Returns the build log for an existing build,
    /// 
    pub fn build_log(&self, build: Entity) -> BuildLog {
        self.world.read_component::<BuildLog>().get(build).cloned().unwrap_or(self.build_log.clone())
    }

    /// Returns compiled runmd data,
    /// 
    pub fn compiled(&self) -> Compiled {
        self.world.system_data::<Compiled>()
    }

    /// Visits the last build,
    /// 
    /// Returns the entity of the last build that was visited,
    /// 
    pub fn visit_last_build(&self, visitor: &mut impl Visitor) -> Option<Entity>{
        if let Some(last) = self.builds.last() {
            self.compiled().visit_build(*last, visitor);
            Some(*last)
        } else {
            None
        }
    }
}

impl PacketHandler for Compiler {
    fn on_packet(&mut self, packet: Packet) -> Result<(), crate::Error> {
        self.block_list.on_packet(packet.clone())?;

        if let Some(built) = self.lazy_build(&packet).ok() {
            trace!("Built packet, {:?}", built);
            self.build_log.index.insert(packet.identifier.commit()?, built);
        }

        Ok(())
    }
}

impl AsRef<World> for Compiler {
    fn as_ref(&self) -> &World {
        &self.world
    }
}

impl AsMut<World> for Compiler {
    fn as_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl Build for Compiler {
    fn build(
        &self,
        lazy_builder: specs::world::LazyBuilder,
    ) -> Result<specs::Entity, crate::Error> {
        let mut log = self.build_log.clone();
        for (ident, block) in self.block_list.blocks() {
            let e = self.lazy_build(block)?;
            log.index.insert(ident.commit()?, e);

            for a in block.roots() {
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
    index: BTreeMap<Identifier, Entity>,
}

impl BuildLog {
    /// Returns a reference to the build log's index,
    /// 
    pub fn index(&self) -> &BTreeMap<Identifier, Entity> {
        &self.index
    }
}
