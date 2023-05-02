use self::linker::LinkerEvents;

use super::parser::Packet;
use super::parser::PacketHandler;
use super::prelude::Visit;
use super::thunk::ThunkUpdate;
use super::thunk::Update;
use super::Documentation;
use super::GetMatches;
use super::Linker;
use super::Listen;
use super::Properties;
use super::Runmd;
use super::ThunkBuild;
use super::ThunkCall;
use super::ThunkCompile;
use super::ThunkListen;
use super::Visitor;
use crate::state::Provider;
use crate::v2::Block;
use crate::v2::BlockList;
use crate::v2::Build;
use crate::v2::Root;
use crate::Error;
use crate::Identifier;
use specs::Builder;
use specs::Component;
use specs::Entity;
use specs::LazyUpdate;
use specs::World;
use specs::WorldExt;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;
use tracing::trace;

mod compiled;
pub use compiled::Build as CompiledBuild;
pub use compiled::Compiled;
pub use compiled::Object;

mod build_log;
pub use build_log::BuildLog;

mod dispatch_ref;
pub use dispatch_ref::DispatchRef;
pub(crate) use dispatch_ref::WorldRef;
pub use dispatch_ref::WorldWrapper;

pub mod linker;

/// Enumeration of different compiler events,
///
pub enum CompilerEvents<'a, T> {
    /// Context when properties of the config block are compiled,
    ///
    Config(&'a Properties),
    /// Context when loading T,
    ///
    Load(&'a T),
}

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
    /// Documentation,
    ///
    documentation: Option<Documentation>,
    /// Packet handlers,
    ///
    packet_handlers: Vec<Box<dyn PacketHandler + Send + Sync>>,
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
        world.register::<ThunkUpdate>();
        world.register::<ThunkListen>();
        world.register::<ThunkCompile>();
        world.register::<LinkerEvents>();
        world.insert(None::<tokio::runtime::Handle>);
        Compiler {
            world,
            block_list: BlockList::default(),
            build_log: BuildLog::default(),
            builds: vec![],
            documentation: None,
            packet_handlers: vec![],
        }
    }

    /// Returns self w/ documentation enabled,
    ///
    pub fn with_docs(mut self) -> Self {
        self.documentation = Some(Documentation::default());
        self
    }

    /// Includes packet handler w/ this compiler,
    ///
    pub fn with_handler(
        mut self,
        packet_handler: impl PacketHandler + Sync + Send + 'static,
    ) -> Self {
        self.packet_handlers.push(Box::new(packet_handler));
        self
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
    /// **Notes**
    ///
    /// - A build for a compiler is any entity w/ a BuildLog component.
    /// - Builds built by a compiler share the same resources.
    ///
    pub fn compile(&mut self) -> Result<Entity, Error> {
        let build = {
            let lzb = self.world.fetch::<LazyUpdate>();
            let lzb = lzb.deref().create_entity(&self.world.entities());

            self.build(lzb)?
        };

        // Clear internal state,
        self.build_log.index_mut().clear();
        self.block_list = BlockList::default();
        self.world.maintain();
        self.builds.push(build);

        Ok(build)
    }

    /// Returns compiled runmd data,
    ///
    pub fn compiled(&self) -> Compiled {
        self.world.system_data::<Compiled>()
    }

    /// Returns the entity of the last build,
    ///
    pub fn last_build(&self) -> Option<&Entity> {
        self.builds.last()
    }

    /// Pushes a new build entity,
    ///
    pub fn push_build(&mut self, entity: Entity) {
        self.builds.push(entity);
    }

    /// Returns a clone of the last build log,
    ///
    pub fn last_build_log(&self) -> Option<BuildLog> {
        if let Some(last) = self.last_build() {
            let compiled = self.compiled();
            compiled.find_build(*last).cloned()
        } else {
            None
        }
    }

    /// Visits the last build,
    ///
    /// Returns the entity of the last build that was visited,
    ///
    pub fn visit_last_build(&self, visitor: &mut impl Visitor) -> Option<Entity> {
        if let Some(last) = self.builds.last() {
            self.compiled().visit_build(*last, visitor);
            Some(*last)
        } else {
            None
        }
    }

    pub fn visit_build(&self, entity: Entity, visitor: &mut impl Visitor) -> Entity {
        self.compiled().visit_build(entity, visitor);
        entity
    }

    /// Visits an object,
    ///
    /// Returns the entity of the object if successfully visited,
    ///
    pub fn visit_object(&self, entity: Entity, visitor: &mut impl Visitor) -> Option<Entity> {
        self.compiled()
            .visit_object(entity, visitor)
            .map(|_| entity)
    }

    /// Updates the last build, if successful returns the entity of the last build,
    ///
    pub fn update_last_build<'a, T, C: Visitor + Update<T>>(
        &'a mut self,
        updater: &mut C,
    ) -> DispatchRef<'a, C> {
        self.visit_last_build(updater)
            .map(|entity| self.compiled().update(entity, updater).map(|_| entity))
            .map(move |result| match result {
                Ok(entity) => {
                    self.as_mut().maintain();
                    self.build_ref(entity)
                }
                Err(err) => err.into(),
            })
            .unwrap_or(DispatchRef::empty())
    }

    /// Visits and updates an object,
    ///
    /// Returns an error if the object no longer exists,
    ///
    pub fn update_object<'a, T, C: Visitor + Update<T> + Default>(
        &'a mut self,
        entity: Entity,
        updater: &mut C,
    ) -> DispatchRef<'a, C> {
        self.visit_object(entity, updater)
            .map(|_| self.compiled().update(entity, updater))
            .map(move |result| match result {
                Ok(_) => {
                    self.as_mut().maintain();
                    self.build_ref(entity)
                }
                Err(err) => err.into(),
            })
            .unwrap_or_default()
    }

    /// Returns a build ref for a given entity,
    ///
    pub fn build_ref<'a, T: Send + Sync + 'a>(&'a mut self, entity: Entity) -> DispatchRef<'a, T> {
        DispatchRef::<'a, T> {
            world_ref: Some(self),
            entity: Some(entity),
            error: None,
            _u: PhantomData,
        }
    }

    pub fn empty_build_ref<'a, T: Send + Sync + 'a>(&'a mut self) -> DispatchRef<'a, T> {
        DispatchRef::<'a, T> {
            world_ref: Some(self),
            entity: None,
            error: None,
            _u: PhantomData,
        }
    }

    /// Creates a linker and links w/ current build log,
    ///
    pub fn link<T>(&mut self, new: T) -> crate::Result<()>
    where
        T: Runmd + Debug,
        for<'a> &'a T: Visit,
        <T as Component>::Storage: Default,
    {
        let builds = { 
            let compiled = self.compiled();
            compiled.state_vec::<crate::v2::prelude::Build>().iter().map(|b| (b.0, b.1.build_log.clone())).collect::<Vec<_>>() 
        };

        for (_, log) in builds.iter().take(2) {
            let dispref = self.empty_build_ref::<T>();

            let mut linker = Linker::new(
                new.clone(), 
                log.clone()
            )
            .activate(dispref);
            
            linker.link()?;
        }

        Ok(())

        // if let Some(log) = self.last_build_log() {
        //     for (i, m, e) in <T as Runmd>::Extensions::get_matches(&log) {
        //         trace!("Linking {:?}", m);

        //         let dispref = self.build_ref::<T>(e);

        //         let mut linker = Linker::new(
        //             new.clone(),
        //             log.clone()
        //         ).activate(dispref);

        //         linker.link()?;
        //     }

        //     Ok(())
        // } else {
        //     Err("No build log to link with".into())
        // }
    }
}

impl PacketHandler for Compiler {
    fn on_packet(&mut self, packet: Packet) -> Result<(), crate::Error> {
        // Ingest documentation from packets,
        if let Some(docs) = self.documentation.as_mut() {
            docs.on_packet(packet.clone())?;
        }

        for p in self.packet_handlers.iter_mut() {
            p.on_packet(packet.clone())?;
        }

        // Route packets to respective blocks,
        self.block_list.on_packet(packet.clone())?;

        // Ignoring errors since at this level we only care about the extension keyword,
        if let Some(built) = self.lazy_build(&packet).ok() {
            trace!("Built extension packet, {:?}", built);
            self.build_log
                .index_mut()
                .insert(packet.identifier.commit()?, built);
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

impl WorldRef for Compiler {}

impl Build for Compiler {
    fn build(
        &self,
        lazy_builder: specs::world::LazyBuilder,
    ) -> Result<specs::Entity, crate::Error> {
        let mut log = self.build_log.clone();
        for (ident, block) in self.block_list.blocks() {
            let e = self.lazy_build(block)?;
            log.index_mut().insert(ident.commit()?, e);

            for a in block.roots() {
                let e = self.lazy_build(a)?;
                log.index_mut().insert(a.ident.commit()?, e);
            }
        }

        Ok(lazy_builder.with(log).build())
    }
}
