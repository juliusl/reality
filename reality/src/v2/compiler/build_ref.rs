use futures::Future;
use specs::Component;
use specs::Entity;
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
pub struct BuildRef<'a, T: 'a, const ENABLE_ASYNC: bool = false> {
    /// The compiler that owns the entity being referenced,
    ///
    pub(super) world_ref: Option<&'a mut dyn WorldRef>,
    /// The entity built by the reality compiler,
    ///
    pub(super) entity: Option<Entity>,
    /// Current error,
    ///
    pub(super) error: Option<Arc<Error>>,
    /// (unused) Alignment + Phantom
    ///
    pub(super) _u: PhantomData<T>,
}

pub trait WorldRef: AsMut<World> + AsRef<World> {}

pub struct WorldWrapper<'a>(&'a mut World);

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

impl<'a, T: 'a, const ENABLE_ASYNC: bool> BuildRef<'a, T, ENABLE_ASYNC> {
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
        if self.error.is_some() {
            self.world_ref.take();
            self.entity.take();
        }
        self
    }

    /// Stores a component w/ the entity in the current reference,
    ///
    /// Note: Ensures that the component being stored is registered first
    ///
    pub fn store<C: Component + 'a>(&mut self, comp: C) -> Result<(), Error>
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
impl<'a, T: Component + 'a, const ENABLE_ASYNC: bool> BuildRef<'a, T, ENABLE_ASYNC> {
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
}

/// API's to work with specs storage through the build ref,
///
impl<'a, T: Component + 'a> BuildRef<'a, T> {
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

    /// Maps component T to component C and inserts C to storage,
    ///
    /// Returns the transmutation of this build reference into a BuildRef<C>,
    ///
    pub fn map_into<C: Component + 'a>(
        self,
        d: impl FnOnce(&T) -> Result<C, Error>,
    ) -> BuildRef<'a, C>
    where
        <C as specs::Component>::Storage: std::default::Default,
    {
        self.map::<C>(d).transmute()
    }

    /// Transmutes this build reference from BuildRef<T> to BuildRef<C>,
    ///
    pub fn transmute<C: Component + 'a>(self) -> BuildRef<'a, C> {
        BuildRef {
            world_ref: self.world_ref,
            entity: self.entity,
            _u: PhantomData,
            error: self.error,
        }
    }

    /// Returns self with Async API enabled,
    ///
    pub fn enable_async(self) -> BuildRef<'a, T, true> {
        BuildRef {
            world_ref: self.world_ref,
            entity: self.entity,
            _u: PhantomData,
            error: self.error,
        }
    }
}

/// Async-version of API's to work with specs storage through the build ref,
///
impl<'a, T: Component + 'a> BuildRef<'a, T, true> {
    /// Write the Component from the build reference, chainable
    ///
    pub async fn write<F>(mut self, d: impl FnOnce(&mut T) -> F) -> BuildRef<'a, T, true>
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
    pub async fn read<F>(mut self, d: impl FnOnce(&T) -> F) -> BuildRef<'a, T, true>
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

    /// Maps component T to component C and inserts C to storage for this entity, chainable
    ///
    pub async fn map<C: Component + 'a, F>(
        mut self,
        d: impl FnOnce(&T) -> F,
    ) -> BuildRef<'a, T, true>
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

    /// Maps component T to component C and inserts C to storage, chainable
    ///
    /// Returns the transmutation of this build reference into a BuildRef<C>,
    ///
    pub async fn map_into<C: Component + 'a, F>(
        self,
        d: impl FnOnce(&T) -> F,
    ) -> BuildRef<'a, C, true>
    where
        <C as specs::Component>::Storage: std::default::Default,
        F: Future<Output = Result<C, Error>>,
    {
        self.map::<C, F>(d).await.transmute()
    }

    /// Transmutes this build reference from BuildRef<T> to BuildRef<C>,
    ///
    pub fn transmute<C: Component + 'a>(self) -> BuildRef<'a, C, true> {
        BuildRef {
            world_ref: self.world_ref,
            entity: self.entity,
            error: self.error,
            _u: PhantomData,
        }
    }

    /// Returns self with async api disabled,
    ///
    pub fn disable_async(self) -> BuildRef<'a, T> {
        BuildRef {
            world_ref: self.world_ref,
            entity: self.entity,
            error: self.error,
            _u: PhantomData,
        }
    }
}

impl<T, const ENABLE_ASYNC: bool> From<Error> for BuildRef<'_, T, ENABLE_ASYNC> {
    fn from(value: Error) -> Self {
        Self {
            world_ref: None,
            entity: None,
            error: Some(Arc::new(value)),
            _u: PhantomData,
        }
    }
}
