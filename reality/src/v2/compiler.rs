use super::Listen;
use super::parser::Packet;
use super::parser::PacketHandler;
use super::thunk::ThunkUpdate;
use super::thunk::Update;
use super::Links;
use super::Properties;
use super::ThunkBuild;
use super::ThunkCall;
use super::ThunkListen;
use super::Visitor;
use crate::v2::Block;
use crate::v2::BlockList;
use crate::v2::Build;
use crate::v2::Root;
use crate::Error;
use crate::Identifier;
use async_trait::async_trait;
use specs::Builder;
use specs::Component;
use specs::Entity;
use specs::HashMapStorage;
use specs::LazyUpdate;
use specs::World;
use specs::WorldExt;
use std::collections::BTreeMap;
use std::ops::Deref;
use tracing::error;
use tracing::trace;

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
        world.register::<ThunkUpdate>();
        world.register::<ThunkListen>();
        world.register::<Links>();
        world.insert(None::<tokio::runtime::Handle>);
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

    /// Updates the last build, if successful returns the entity of the last build,
    ///
    pub fn update_last_build<C: Visitor + Update>(&mut self, updater: &mut C) -> Option<Entity> {
        self.visit_last_build(updater)
            .and_then(|l| match self.compiled().update(l, updater) {
                Ok(_) => Some(l),
                Err(err) => {
                    error!("Could not update, {err}");
                    None
                }
            })
            .map(|l| {
                self.as_mut().maintain();
                l
            })
    }

    /// Visits and updates an object,
    ///
    /// Returns an error if the object no longer exists,
    ///
    pub fn update_object<C: Visitor + Update>(
        &mut self,
        obj_entity: Entity,
        updater: &mut C,
    ) -> Result<(), crate::Error> {
        if self.compiled().visit_object(obj_entity, updater).is_some() {
            self.compiled().update(obj_entity, updater)
        } else {
            Err("Object did not exist".into())
        }
    }
}

impl PacketHandler for Compiler {
    fn on_packet(&mut self, packet: Packet) -> Result<(), crate::Error> {
        self.block_list.on_packet(packet.clone())?;

        if let Some(built) = self.lazy_build(&packet).ok() {
            trace!("Built packet, {:?}", built);
            self.build_log
                .index
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
    /// Index mapping identifiers into their current entities,
    ///
    index: BTreeMap<Identifier, Entity>,
}

impl BuildLog {
    /// Returns a reference to the build log's index,
    ///
    pub fn index(&self) -> &BTreeMap<Identifier, Entity> {
        &self.index
    }

    /// Searches for an object by identifier,
    ///
    pub fn get(&self, identifier: &Identifier) -> Result<Entity, Error> {
        let search = identifier.commit()?;

        self.index()
            .get(&search)
            .map(|e| Ok(e))
            .unwrap_or(Err(
                format!("Could not find object w/ {:#}", identifier).into()
            ))
            .copied()
    }

    /// Queries the index w/ a string interpolation pattern,
    ///
    /// Returns the original identifier, interpolation result, and the found entity w/ for each successful interpolation,
    ///
    pub fn search(
        &self,
        pat: impl Into<String>,
    ) -> impl Iterator<Item = (&Identifier, BTreeMap<String, String>, &Entity)> {
        let pat = pat.into();
        self.index().iter().filter_map(move |(ident, entity)| {
            if let Some(map) = ident.interpolate(&pat) {
                Some((ident, map, entity))
            } else {
                None
            }
        })
    }

    /// Updates a property from src properties, where the ident is the property name, and the parent of the ident
    /// is the ident of the object whose properties need to be updated,
    ///
    pub fn update_property(&self, src: &Properties, lazy_update: &LazyUpdate) {
        let property_name = src.owner().to_string();
        let property = src[&property_name].clone();
        if let Some(parent) = src
            .owner()
            .parent()
            .and_then(|p| p.commit().ok())
            .and_then(|p| self.index.get(&p))
            .cloned()
        {
            lazy_update.exec_mut(move |world| {
                if let Some(p) = world.write_component::<Properties>().get_mut(parent) {
                    p.set(property_name, property);
                }
            })
        }
    }
}

#[async_trait]
impl Listen for BuildLog {
    async fn listen(&self, properties: Properties, lazy_update: &LazyUpdate) -> Result<(), Error> {
        self.update_property(&properties, lazy_update);

        Ok(())
    }
}
