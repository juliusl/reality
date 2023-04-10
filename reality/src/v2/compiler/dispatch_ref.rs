use futures::Future;
use specs::world::LazyBuilder;
use specs::Component;
use specs::Entity;
use specs::Join;
use specs::LazyUpdate;
use specs::World;
use specs::WorldExt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use crate::Error;

/// Struct for working w/ a compiler's build log,
///
/// Provides an API for working with the Component created from a compiled build w/o the storage boilerplate,
///
#[derive(Default)]
pub struct DispatchRef<'a, T: Send + Sync + 'a, const ENABLE_ASYNC: bool = false> {
    /// The compiler that owns the entity being referenced,
    ///
    pub(super) world_ref: Option<&'a mut (dyn WorldRef + Send + Sync)>,
    /// The entity built by the reality compiler,
    ///
    pub(crate) entity: Option<Entity>,
    /// Current error,
    ///
    pub(super) error: Option<Arc<Error>>,
    /// Unused
    ///
    pub(super) _u: PhantomData<T>,
}

/// Super trait to get a reference to world,
///
pub trait WorldRef: AsMut<World> + AsRef<World> {}

impl<'a, T: Send + Sync + 'a, const ENABLE_ASYNC: bool> DispatchRef<'a, T, ENABLE_ASYNC> {
    /// Returns a new build ref,
    ///
    pub fn new(entity: Entity, world_ref: &'a mut (dyn WorldRef + Send + Sync)) -> Self {
        Self {
            world_ref: Some(world_ref),
            entity: Some(entity),
            error: None,
            _u: PhantomData,
        }
    }

    /// Returns an empty build ref,
    ///
    pub fn empty() -> Self {
        Self {
            world_ref: None,
            entity: None,
            error: None,
            _u: PhantomData,
        }
    }
    /// Returns the self as Result,
    ///
    /// Note: Can be used to check for errors before moving to the next function in the chain,
    ///
    pub fn result(self) -> Result<Self, Error> {
        if let Some(err) = self.error.as_ref() {
            Err(err.deref().clone())
        } else {
            Ok(self)
        }
    }

    /// Check if an error is set,
    ///
    fn check(mut self) -> Self {
        if self
            .error
            .as_ref()
            .filter(|e| e.deref().deref().as_ref() != Error::skip().as_ref())
            .is_some()
        {
            self.world_ref.take();
            self.entity.take();
        } else {
            self.error.take();
        }
        
        self
    }

    /// Stores a component w/ the entity in the current reference,
    ///
    /// Note: Ensures that the component being stored is registered first
    ///
    pub fn store<C: Component>(&mut self, comp: C) -> Result<(), Error>
    where
        <C as specs::Component>::Storage: std::default::Default,
    {
        if let Some(entity) = self.entity {
            if let Some(result) = self.world_ref.as_mut().map(|c| {
                let world = c.as_mut();
                world.register::<C>();
                world.write_component().insert(entity, comp)
            }) {
                result?;
            }
        }

        Ok(())
    }
}

/// (Internal) Common Component storage-access functions
///
impl<'a, T: Send + Sync + Component + 'a, const ENABLE_ASYNC: bool>
    DispatchRef<'a, T, ENABLE_ASYNC>
{
    /// Dispatches changes to world storage via lazy-update and calls .maintain(),
    ///
    pub fn dispatch(
        &mut self,
        map: impl FnOnce(&T, &LazyUpdate) -> Result<(), Error>,
    ) -> Result<(), Error>
    where
        <T as specs::Component>::Storage: std::default::Default,
    {
        if let (Some(world), Some(entity)) = (self.world_ref.as_mut(), self.entity) {
            world.as_mut().register::<T>();
            {
                world
                    .as_ref()
                    .read_component::<T>()
                    .get(entity)
                    .map(|c| {
                        let lazy_update = world.as_ref().read_resource::<LazyUpdate>();
                        map(c, &lazy_update)
                    })
                    .unwrap_or(Ok(()))?;
            }
            world.as_mut().maintain();
            Ok(())
        } else {
            Err(format!("Could not dispatch changes, {:?}", self.error).into())
        }
    }

    /// Dispatches into a new build_ref of component C forking into a new entity returned from map,
    ///
    pub fn fork_into<C: Component + Send + Sync>(
        mut self,
        map: impl FnOnce(&T, LazyBuilder) -> Result<Entity, Error>,
    ) -> Result<DispatchRef<'a, C, ENABLE_ASYNC>, Error>
    where
        <T as specs::Component>::Storage: std::default::Default,
        <C as specs::Component>::Storage: std::default::Default,
    {
        {
            if let Some(world) = self.world_ref.as_mut() {
                world.as_mut().register::<T>();
            }
        }

        if let (Some(world), Some(entity)) = (self.world_ref, self.entity) {
            let next = world.as_ref().read_component::<T>().get(entity).map(|c| {
                let lazy_update = world.as_ref().read_resource::<LazyUpdate>();
                let lazy_builder = lazy_update.create_entity(&world.as_ref().entities());
                map(c, lazy_builder)
            });

            match next {
                Some(Ok(next)) => {
                    return Ok(DispatchRef {
                        world_ref: Some(world),
                        entity: Some(next),
                        error: None,
                        _u: PhantomData,
                    });
                }
                _ => {}
            }
        }

        Err("Could not fork_into new build ref".into())
    }

    /// Forks map(T, C) -> into C,
    ///
    pub fn fork_into_with<C: Component + Send + Sync>(
        mut self,
        map: impl FnOnce(&T, &C, LazyBuilder) -> Result<Entity, Error>,
    ) -> Result<DispatchRef<'a, C, ENABLE_ASYNC>, Error>
    where
        <T as specs::Component>::Storage: std::default::Default,
        <C as specs::Component>::Storage: std::default::Default,
    {
        {
            if let Some(world) = self.world_ref.as_mut() {
                world.as_mut().register::<T>();
            }
        }

        if let (Some(world), Some(entity)) = (self.world_ref, self.entity) {
            let next = (
                &world.as_ref().read_component::<T>(),
                &world.as_ref().read_component::<C>(),
            )
                .join()
                .get(entity, &world.as_ref().entities())
                .map(|(t, c)| {
                    let lazy_update = world.as_ref().read_resource::<LazyUpdate>();
                    let lazy_builder = lazy_update.create_entity(&world.as_ref().entities());
                    map(t, c, lazy_builder)
                });

            match next {
                Some(Ok(next)) => {
                    return Ok(DispatchRef {
                        world_ref: Some(world),
                        entity: Some(next),
                        error: None,
                        _u: PhantomData,
                    });
                }
                _ => {}
            }
        }

        Err("Could not fork_into_with new build ref".into())
    }

    /// Map a component T to C w/ read access to T
    ///
    fn map_entity<C>(&self, map: impl FnOnce(&T) -> C) -> Option<C> {
        if let Some(entity) = self.entity {
            self.world_ref
                .as_ref()
                .map(|c| c.as_ref().read_component::<T>())
                .and_then(|s| s.get(entity).map(map))
        } else {
            None
        }
    }

    /// Map a component T to C w/ mut access to T
    ///
    fn map_entity_mut<C>(&self, map: impl FnOnce(&mut T) -> C) -> Option<C> {
        if let Some(entity) = self.entity {
            self.world_ref
                .as_ref()
                .map(|c| c.as_ref().write_component::<T>())
                .and_then(|mut s| s.get_mut(entity).map(map))
        } else {
            None
        }
    }

    /// Map a component (&mut T, &C) w/ map returning R,
    ///
    fn map_entity_with<C: Component + Send + Sync + 'a, R>(
        &self,
        map: impl FnOnce(&T, &C) -> R,
    ) -> Option<R> {
        if let Some(entity) = self.entity {
            self.world_ref
                .as_ref()
                .map(|c| {
                    (
                        c.as_ref().entities(),
                        c.as_ref().read_component::<T>(),
                        c.as_ref().read_component::<C>(),
                    )
                })
                .and_then(|(a, b, c)| (&b, &c).join().get(entity, &a).map(|(a, b)| map(a, b)))
        } else {
            None
        }
    }

    /// Map a component (&mut T, &C) w/ map returning R,
    ///
    fn map_entity_mut_with<C: Component + Send + Sync + 'a, R>(
        &self,
        map: impl FnOnce(&mut T, &C) -> R,
    ) -> Option<R> {
        if let Some(entity) = self.entity {
            self.world_ref
                .as_ref()
                .map(|c| {
                    (
                        c.as_ref().entities(),
                        c.as_ref().write_component::<T>(),
                        c.as_ref().read_component::<C>(),
                    )
                })
                .and_then(|(a, mut b, c)| {
                    (&mut b, &c).join().get(entity, &a).map(|(a, b)| map(a, b))
                })
        } else {
            None
        }
    }
}

/// API's to work with specs storage through the build ref,
///
impl<'a, T: Send + Sync + Component + 'a> DispatchRef<'a, T> {
    /// Write the Component from the build reference, chainable
    ///
    pub fn write(mut self, d: impl FnOnce(&mut T) -> Result<(), Error>) -> Self {
        if let Some(Err(error)) = self.map_entity_mut(d) {
            self.error = Some(error.into());
        }

        self.check()
    }

    /// Read the Component from the build reference, chainable
    ///
    pub fn read(mut self, d: impl FnOnce(&T) -> Result<(), Error>) -> Self {
        if let Some(Err(error)) = self.map_entity(d) {
            self.error = Some(error.into());
        }

        self.check()
    }

    /// Write the Component from the build reference, chainable
    ///
    pub fn write_with<C: Component + Send + Sync + 'a>(
        mut self,
        d: impl FnOnce(&mut T, &C) -> Result<(), Error>,
    ) -> Self {
        if let Some(Err(error)) = self.map_entity_mut_with(d) {
            self.error = Some(error.into());
        }

        self.check()
    }

    /// Read the Component from the build reference, chainable
    ///
    pub fn read_with<C: Component + Send + Sync + 'a>(
        mut self,
        d: impl FnOnce(&T, &C) -> Result<(), Error>,
    ) -> Self {
        if let Some(Err(error)) = self.map_entity_with(d) {
            self.error = Some(error.into());
        }

        self.check()
    }

    /// Maps component T to component C and inserts C to storage, chainable
    ///
    pub fn map<C: Component + 'a>(mut self, d: impl FnOnce(&T) -> Result<C, Error>) -> Self
    where
        <C as specs::Component>::Storage: std::default::Default,
    {
        match self.map_entity(d) {
            Some(Ok(next)) => {
                if let Err(error) = self.store(next) {
                    self.error = Some(error.into());
                }
            }
            Some(Err(err)) => {
                self.error = Some(err.into());
            }
            _ => {}
        }

        self.check()
    }

    /// Maps component (&T, &C) to component C and inserts C to storage, chainable
    ///
    pub fn map_with<C: Component + Send + Sync + 'a>(
        mut self,
        d: impl FnOnce(&T, &C) -> Result<C, Error>,
    ) -> Self
    where
        <C as specs::Component>::Storage: std::default::Default,
    {
        match self.map_entity_with(d) {
            Some(Ok(next)) => {
                if let Err(error) = self.store(next) {
                    self.error = Some(error.into());
                }
            }
            Some(Err(err)) => {
                self.error = Some(err.into());
            }
            _ => {}
        }

        self.check()
    }

    /// Maps component T to component C and inserts C to storage for the entity being referenced,
    ///
    /// Returns the transmutation of this build reference into a BuildRef<C>,
    ///
    pub fn map_into<C: Send + Sync + Component + 'a>(
        self,
        d: impl FnOnce(&T) -> Result<C, Error>,
    ) -> DispatchRef<'a, C>
    where
        <C as specs::Component>::Storage: std::default::Default,
    {
        self.map::<C>(d).transmute()
    }

    /// Maps component T to component C and inserts C to storage for the entity being referenced,
    ///
    /// Returns the transmutation of this build reference into a BuildRef<C>,
    ///
    pub fn map_into_with<C: Send + Sync + Component + 'a>(
        self,
        d: impl FnOnce(&T, &C) -> Result<C, Error>,
    ) -> DispatchRef<'a, C>
    where
        <C as specs::Component>::Storage: std::default::Default,
    {
        self.map_with::<C>(d).transmute()
    }

    /// Transmutes this build reference from BuildRef<T> to BuildRef<C>,
    ///
    pub fn transmute<C: Send + Sync + Component + 'a>(self) -> DispatchRef<'a, C> {
        DispatchRef {
            world_ref: self.world_ref,
            entity: self.entity,
            _u: PhantomData,
            error: self.error,
        }
    }

    /// Returns self with Async API enabled,
    ///
    pub fn enable_async(self) -> DispatchRef<'a, T, true> {
        DispatchRef {
            world_ref: self.world_ref,
            entity: self.entity,
            _u: PhantomData,
            error: self.error,
        }
    }
}

/// Async-version of API's to work with specs storage through the build ref,
///
impl<'a, T: Send + Sync + Component + 'a> DispatchRef<'a, T, true> {
    /// Write the Component from the build reference, chainable
    ///
    pub async fn write<F>(mut self, d: impl FnOnce(&mut T) -> F) -> DispatchRef<'a, T, true>
    where
        F: Future<Output = Result<(), Error>>,
    {
        if let Some(f) = self.map_entity_mut(d) {
            if let Err(error) = f.await {
                self.error = Some(error.into());
            }
        }

        self.check()
    }

    /// Read the Component from the build reference, chainable
    ///
    pub async fn read<F>(mut self, d: impl FnOnce(&T) -> F) -> DispatchRef<'a, T, true>
    where
        F: Future<Output = Result<(), Error>>,
    {
        if let Some(f) = self.map_entity(d) {
            if let Err(error) = f.await {
                self.error = Some(error.into());
            }
        }

        self.check()
    }

    /// Write the Component (&mut T, &C) from the build reference, chainable
    ///
    pub async fn write_with<C: Component + Send + Sync + 'a, F>(
        mut self,
        d: impl FnOnce(&mut T, &C) -> F,
    ) -> DispatchRef<'a, T, true>
    where
        F: Future<Output = Result<(), Error>>,
    {
        if let Some(f) = self.map_entity_mut_with(d) {
            if let Err(error) = f.await {
                self.error = Some(error.into());
            }
        }

        self.check()
    }

    /// Read the Component (T, C) from the build reference, chainable
    ///
    pub async fn read_with<C: Component + Send + Sync + 'a, F>(
        mut self,
        d: impl FnOnce(&T, &C) -> F,
    ) -> DispatchRef<'a, T, true>
    where
        F: Future<Output = Result<(), Error>>,
    {
        if let Some(f) = self.map_entity_with(d) {
            if let Err(error) = f.await {
                self.error = Some(error.into());
            }
        }

        self.check()
    }

    /// Maps component T to component C and inserts C to storage for this entity, chainable
    ///
    pub async fn map<C: Component + Send + Sync + 'a, F>(
        mut self,
        d: impl FnOnce(&T) -> F,
    ) -> DispatchRef<'a, T, true>
    where
        <C as specs::Component>::Storage: std::default::Default,
        F: Future<Output = Result<C, Error>>,
    {
        if let Some(next) = self.map_entity(d) {
            match next.await {
                Ok(next) => {
                    if let Err(err) = self.store(next) {
                        self.error = Some(err.into());
                    }
                }
                Err(err) => {
                    self.error = Some(err.into());
                }
            }
        }

        self.check()
    }

    /// Maps component (&T, &C) to component C and inserts C to storage for this entity, chainable
    ///
    pub async fn map_with<C: Component + Send + Sync + 'a, F>(
        mut self,
        d: impl FnOnce(&T, &C) -> F,
    ) -> DispatchRef<'a, T, true>
    where
        <C as specs::Component>::Storage: std::default::Default,
        F: Future<Output = Result<C, Error>>,
    {
        if let Some(next) = self.map_entity_with(d) {
            match next.await {
                Ok(next) => {
                    if let Err(err) = self.store(next) {
                        self.error = Some(err.into());
                    }
                }
                Err(err) => {
                    self.error = Some(err.into());
                }
            }
        }

        self.check()
    }

    /// Maps component T to component C and inserts C to storage, chainable
    ///
    /// Returns the transmutation of this build reference into a BuildRef<C>,
    ///
    pub async fn map_into<C: Send + Sync + Component + 'a, F>(
        self,
        d: impl FnOnce(&T) -> F,
    ) -> DispatchRef<'a, C, true>
    where
        <C as specs::Component>::Storage: std::default::Default,
        F: Future<Output = Result<C, Error>>,
    {
        self.map::<C, F>(d).await.transmute()
    }

    /// Maps component (&T, &C) to component C and inserts C to storage, chainable
    ///
    /// Returns the transmutation of this build reference into a BuildRef<C>,
    ///
    pub async fn map_into_with<C: Send + Sync + Component + 'a, F>(
        self,
        d: impl FnOnce(&T, &C) -> F,
    ) -> DispatchRef<'a, C, true>
    where
        <C as specs::Component>::Storage: std::default::Default,
        F: Future<Output = Result<C, Error>>,
    {
        self.map_with::<C, F>(d).await.transmute()
    }

    /// Transmutes this build reference from BuildRef<T> to BuildRef<C>,
    ///
    pub fn transmute<C: Send + Sync + Component + 'a>(self) -> DispatchRef<'a, C, true> {
        DispatchRef {
            world_ref: self.world_ref,
            entity: self.entity,
            error: self.error,
            _u: PhantomData,
        }
    }

    /// Returns self with async api disabled,
    ///
    pub fn disable_async(self) -> DispatchRef<'a, T> {
        DispatchRef {
            world_ref: self.world_ref,
            entity: self.entity,
            error: self.error,
            _u: PhantomData,
        }
    }

    /// (Async) Dispatches changes to world storage via lazy-update and calls .maintain(),
    ///
    pub async fn async_dispatch<F>(
        &mut self,
        map: impl FnOnce(&T, &LazyUpdate) -> F,
    ) -> Result<(), Error>
    where
        <T as specs::Component>::Storage: std::default::Default,
        F: Future<Output = Result<(), Error>> + Send,
    {
        if let (Some(world), Some(entity)) = (self.world_ref.as_mut(), self.entity) {
            world.as_mut().register::<T>();
            {
                if let Some(f) = world.as_ref().read_component::<T>().get(entity).map(|c| {
                    let lazy_update = world.as_ref().read_resource::<LazyUpdate>();
                    map(c, &lazy_update)
                }) {
                    f.await?;
                }
            }
            world.as_mut().maintain();
            Ok(())
        } else {
            Err(format!("Could not dispatch changes, {:?}", self.error).into())
        }
    }
}

impl<'a, T: Send + Sync + 'a, const ENABLE_ASYNC: bool> From<Error>
    for DispatchRef<'a, T, ENABLE_ASYNC>
{
    fn from(value: Error) -> Self {
        Self {
            world_ref: None,
            entity: None,
            error: Some(Arc::new(value)),
            _u: PhantomData,
        }
    }
}

/// Wrapper-struct for implementing WorldRef trait,
///
pub struct WorldWrapper<'a>(&'a mut World);

impl<'a> WorldWrapper<'a> {
    /// Returns a build ref for an entity,
    ///
    pub fn get_ref<T: Component + Sync + Send>(&'a mut self, entity: Entity) -> DispatchRef<'a, T> {
        DispatchRef {
            world_ref: Some(self),
            entity: Some(entity),
            error: None,
            _u: PhantomData,
        }
    }
}

impl<'a> AsRef<World> for WorldWrapper<'a> {
    fn as_ref(&self) -> &World {
        self.0
    }
}

impl<'a> AsMut<World> for WorldWrapper<'a> {
    fn as_mut(&mut self) -> &mut World {
        self.0
    }
}

impl<'a> WorldRef for WorldWrapper<'a> {}

/// Returns a wrapper over world that implements WorldRef
///
impl<'a> From<&'a mut World> for WorldWrapper<'a> {
    fn from(value: &'a mut World) -> Self {
        Self(value)
    }
}