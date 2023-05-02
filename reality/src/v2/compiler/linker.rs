use std::fmt::Debug;

use specs::Component;
use specs::Entities;
use specs::Entity;
use specs::Join;
use specs::ReadStorage;
use specs::RunNow;
use specs::System;
use specs::VecStorage;
use specs::WorldExt;
use specs::WriteStorage;
use specs::shred::SetupHandler;

use tracing::trace;

use crate::v2::visitor::Visit;
use crate::v2::GetMatches;
use crate::v2::Properties;
use crate::v2::Runmd;
use crate::v2::Visitor;
use crate::Result;

use super::BuildLog;
use super::CompilerEvents;
use super::DispatchRef;

/// Enumeration of linker events,
///
#[derive(Component)]
#[storage(VecStorage)]
pub enum LinkerEvents {
    /// Entity has config to add,
    /// 
    AddConfig(Properties),
    /// Create component,
    /// 
    Create(Properties),
}

/// A Linker wraps a runmd component T and scans the build log index for entities that are related,
///
/// Entities are selected via the Runmd::Extensions associated type that implements GetMatches.
///
/// When the Runmd trait is derived, there are 3 distinct patterns that are relevant to an extension.
///
/// Given some extension named Plugin,
///
/// 1) `PluginRoot`  : This entity is the root of the extension symbol
/// 2) `PluginConfig`: This entity is a configuration of the extension
/// 3) `Plugin`      : This entity is a usage of the extension
///
/// *Root and *Config entities are defined and found in the "root" block
///
pub struct Linker<'a, T>
where
    T: Runmd,
    for<'b> &'b T: Visit,
    <T as Component>::Storage: Default,
{
    /// The configuration to use when creating a new instance of T,
    ///
    new: T,
    /// Build log w/ mapping from identifiers <-> Entity ID in storage,
    ///
    build_log: BuildLog,
    /// The dispatch ref w/ access to storage,
    ///
    dispatch: Option<DispatchRef<'a, T>>,
}

impl<'a, T> Linker<'a, T>
where
    T: Runmd + std::fmt::Debug,
    for<'b> &'b T: Visit,
    <T as Component>::Storage: Default,
{
    /// Creates a new Linker,
    ///
    pub fn new(new: T, build_log: BuildLog) -> Self {
        Self {
            new,
            build_log,
            dispatch: None,
        }
    }

    /// Activates the linker by providing a dispatch ref to storage,
    ///
    pub fn activate(self, dispatch: DispatchRef<'a, T>) -> Self {
        Self {
            new: self.new,
            dispatch: Some(dispatch),
            build_log: self.build_log,
        }
    }

    /// Begin linking process
    ///
    pub fn link(&mut self) -> Result<()> {
        // Scan build log for relevant types for config,
        //
        let matches = <T::Extensions as GetMatches>::get_matches(&self.build_log);

        if let Some(d) = self.dispatch.as_mut().and_then(|d| d.world_ref.as_mut()) {
            d.as_mut().register::<T>();
        }

        // Configure
        // 
        for (id, m, _) in matches {
            // Visiting the identifier will set the entity,
            //
            self.visit_identifier(&id);

            if let Some(mut dispatch) = self.dispatch.take() {
                let load_m = m.clone();
                dispatch.store(self.new.clone())?;
                self.dispatch = Some(
                    dispatch
                        .map(move |p| {
                            let mut properties = Properties::empty();
                            // load_m.visit(CompilerEvents::Load(p), properties);
                            <T::Extensions as Visit<CompilerEvents<T>>>::visit(&load_m, CompilerEvents::Load(p), &mut properties)?;
                            Ok(LinkerEvents::Create(properties))
                        })
                        .transmute::<Properties>()
                        .map(move |p| {
                            let mut properties = Properties::empty();
                            m.visit(CompilerEvents::Config(p), &mut properties)?;
                            Ok(LinkerEvents::AddConfig(properties))
                        })
                        .transmute(),
                );
            }
        }

        // Prepare the base type
        if let Some(mut dispatch) = self.dispatch.take() {
            dispatch.dispatch_mut(|t, lz| {
                let base = t.clone();
                lz.exec_mut(move |w| {
                    let mut configuring = BaseType::new(base);
                    configuring.run_now(w);
                    for (e, mut i) in configuring.instances {
                        // Finish completing instances
                        let mut ip = Properties::empty();
                        <&T as Visit>::visit(&&i, (), &mut ip).ok();
                        i.visit_properties(&ip);

                        // Write finished product
                        w.write_component().insert(e, i).ok();
                    }
                });
                Ok(())
            })?;

            self.dispatch = Some(
                dispatch
            );
        }

        Ok(())
    }
}

impl<'a, T> Visitor for Linker<'a, T>
where
    T: Runmd,
    for<'b> &'b T: Visit,
    <T as Component>::Storage: Default,
{
    /// Visiting the identifier will update the current entity in the dispatch ref,
    ///
    fn visit_identifier(&mut self, identifier: &crate::Identifier) {
        if let Some(e) = self.build_log.try_get(identifier).ok() {
            self.dispatch = self
                .dispatch
                .take()
                .map(|d| d.with_entity(e).map(|_| Ok(identifier.clone())));
        }
    }

    fn visit_extension(&mut self, identifier: &crate::Identifier) {
        self.dispatch = self.dispatch.take().map(|d| {
            d.write(|v| {
                v.visit_extension(identifier);
                Ok(())
            })
        })
    }

    fn visit_property(&mut self, name: &str, property: &crate::v2::Property) {
        self.dispatch = self.dispatch.take().map(|d| {
            d.write(|v| {
                v.visit_property(name, property);
                Ok(())
            })
        })
    }
}

impl<'a, T> SetupHandler<Option<T>> for Linker<'a, T>
where
    T: Runmd,
    for<'b> &'b T: Visit,
    <T as Component>::Storage: Default,
{
    fn setup(world: &mut specs::World) {
        world.register::<LinkerEvents>();
        world.insert(None::<T>);
    }
}

/// Struct containing the base type and instances created by the linker,
/// 
#[derive(Debug)]
struct BaseType<T>
where
    T: Runmd,
    for<'b> &'b T: Visit,
    <T as Component>::Storage: Default 
{
    base: T,
    instances: Vec<(Entity, T)>,
}

impl<T> BaseType<T> 
where
    T: Runmd,
    for<'b> &'b T: Visit,
    <T as Component>::Storage: Default  
{
    /// Returns a new struct,
    /// 
    pub(super) const fn new(base: T) -> Self {
        Self { base, instances: vec![] }
    }
}

impl<'a, T> System<'a> for BaseType<T>
where
    T: Runmd + Debug,
    for<'b> &'b T: Visit,
    <T as Component>::Storage: Default,
{
    type SystemData = (Entities<'a>, WriteStorage<'a, LinkerEvents>, ReadStorage<'a, Properties>);

    fn run(&mut self, (entities, mut events, properties): Self::SystemData) {
        let mut create_queue = vec![];
        for (e, p, event) in (&entities, &properties, events.drain()).join() {
            match event {
                LinkerEvents::AddConfig(config) => {
                    trace!("Adding config config from {:?}", e);
                    self.base.visit_properties(&config);
                },
                LinkerEvents::Create(mut properties) => {
                    trace!("Creating for {:?}", e);
                    // The final config will be performed in the create queue
                    for (n, p) in p.iter_properties() {
                        properties.visit_property(n, p);
                    }
                    create_queue.push((e, properties));
                }
            }
        }

        for (e, properties) in create_queue.iter() {
            trace!("creating -- {:?}", e);
            let mut base = self.base.clone();
            base.visit_properties(properties);

            trace!("created -- {:#?}", base);
            self.instances.push((*e, base));
        }
    }
}