use std::ops::Deref;
use std::sync::Arc;

use super::Compiler;
use crate::Error;
use futures::Future;
use specs::Component;
use specs::Entity;
use specs::WorldExt;
use tracing::error;

/// Struct for working w/ a compiler's build log,
///
/// Provides an API for working with the Component created from a compiled build w/o the storage boilerplate,
///
#[derive(Default)]
pub struct BuildRef<'a, T, const ENABLE_ASYNC: bool = false> {
    /// The compiler that owns the entity being referenced,
    /// 
    pub(super) compiler: Option<&'a mut Compiler>,
    /// The entity built by the reality compiler,
    /// 
    pub(super) entity: Option<Entity>,
    /// Current error,
    /// 
    pub(super) error: Option<Arc<Error>>,
    /// (unused) Struct alignment + Phantom
    /// 
    pub(super) _u: Option<fn(T)>,
}

impl<T, const ENABLE_ASYNC: bool> BuildRef<'_, T, ENABLE_ASYNC> {
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

    fn check(mut self) -> Self {
        if self.error.is_some() {
            self.compiler.take();
            self.entity.take();
        }
        self
    }
}

/// API's to work with specs storage through the build ref,
/// 
impl<'a, T: Component> BuildRef<'a, T> {
    /// Write the Component from the build reference, chainable
    ///
    pub fn write(mut self, d: impl FnOnce(&mut T) -> Result<(), Error>) -> Self {
        match (self.compiler.as_mut(), self.entity) {
            (Some(compiler), Some(entity)) => {
                let world = compiler.as_ref();
                if let Some(Err(err)) = world.write_component::<T>().get_mut(entity).map(d) {
                    self.error = Some(Arc::new(err));
                }
            }
            _ => {}
        }

        self.check()
    }

    /// Read the Component from the build reference, chainable
    ///
    pub fn read(mut self, d: impl FnOnce(&T) -> Result<(), Error>) -> Self {
        match (self.compiler.as_ref(), self.entity) {
            (Some(compiler), Some(entity)) => {
                let world = compiler.as_ref();
                if let Some(Err(err)) = world.read_component::<T>().get(entity).map(d) {
                    self.error = Some(Arc::new(err));
                }
            }
            _ => {}
        }

        self.check()
    }

    /// Maps component T to component C and inserts C to storage, chainable
    ///
    pub fn map<C: Component>(mut self, d: impl FnOnce(&T) -> Result<C, Error>) -> Self
    where
        <C as specs::Component>::Storage: std::default::Default,
    {
        match (self.compiler.as_mut(), self.entity) {
            (Some(compiler), Some(entity)) => {
                let world = compiler.as_mut();
                world.register::<C>();

                match world.read_component::<T>().get(entity).map(d) {
                    Some(Ok(next)) => {
                        let _ = world.write_component().insert(entity, next).map_err(|e| {
                            error!("Error writing component, {e}");
                        });
                    }
                    Some(Err(err)) => {
                        self.error = Some(Arc::new(err));
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        self.check()
    }

    /// Maps component T to component C and inserts C to storage,
    ///
    /// Returns the transmutation of this build reference into a BuildRef<C>,
    ///
    pub fn map_into<C: Component>(self, d: impl FnOnce(&T) -> Result<C, Error>) -> BuildRef<'a, C>
    where
        <C as specs::Component>::Storage: std::default::Default,
    {
        self.map::<C>(d).transmute()
    }

    /// Transmutes this build reference from BuildRef<T> to BuildRef<C>,
    ///
    pub fn transmute<C: Component>(self) -> BuildRef<'a, C> {
        BuildRef {
            compiler: self.compiler,
            entity: self.entity,
            _u: None,
            error: self.error,
        }
    }

    /// Returns self with Async API enabled,
    ///
    pub fn enable_async(self) -> BuildRef<'a, T, true> {
        BuildRef {
            compiler: self.compiler,
            entity: self.entity,
            _u: None,
            error: self.error,
        }
    }
}

/// Async-version of API's to work with specs storage through the build ref,
/// 
impl<'a, T: Component> BuildRef<'a, T, true> {
    /// Write the Component from the build reference, chainable
    ///
    pub async fn write<F>(mut self, d: impl FnOnce(&mut T) -> F) -> BuildRef<'a, T, true>
    where
        F: Future<Output = Result<(), Error>>,
    {
        match (self.compiler.as_mut(), self.entity) {
            (Some(compiler), Some(entity)) => {
                let world = compiler.as_ref();
                if let Some(f) = world.write_component::<T>().get_mut(entity).map(d) {
                    if let Err(err) = f.await {
                        self.error = Some(Arc::new(err));
                    }
                }
            }
            _ => {}
        }

        self.check()
    }

    /// Read the Component from the build reference, chainable
    ///
    pub async fn read<F>(mut self, d: impl FnOnce(&T) -> F) -> BuildRef<'a, T, true>
    where
        F: Future<Output = Result<(), Error>>,
    {
        match (self.compiler.as_ref(), self.entity) {
            (Some(compiler), Some(entity)) => {
                let world = compiler.as_ref();
                if let Some(f) = world.read_component::<T>().get(entity).map(d) {
                    if let Err(err) = f.await {
                        self.error = Some(Arc::new(err));
                    }
                }
            }
            _ => {}
        }

        self.check()
    }

    /// Maps component T to component C and inserts C to storage, chainable
    ///
    pub async fn map<C: Component, F>(mut self, d: impl FnOnce(&T) -> F) -> BuildRef<'a, T, true>
    where
        <C as specs::Component>::Storage: std::default::Default,
        F: Future<Output = Result<C, Error>>,
    {
        match (self.compiler.as_mut(), self.entity) {
            (Some(compiler), Some(entity)) => {
                let world = compiler.as_mut();
                world.register::<C>();

                if let Some(next) = world.read_component::<T>().get(entity).map(d) {
                    match next.await {
                        Ok(next) => {
                            let _ = world.write_component().insert(entity, next).map_err(|e| {
                                error!("Error writing component, {e}");
                            });
                        }
                        Err(err) => {
                            self.error = Some(Arc::new(err));
                        }
                    }
                }
            }
            _ => {}
        }

        self.check()
    }

    /// Maps component T to component C and inserts C to storage, chainable
    ///
    /// Returns the transmutation of this build reference into a BuildRef<C>,
    ///
    pub async fn map_into<C: Component, F>(self, d: impl FnOnce(&T) -> F) -> BuildRef<'a, C, true>
    where
        <C as specs::Component>::Storage: std::default::Default,
        F: Future<Output = Result<C, Error>>,
    {
        self.map::<C, F>(d).await.transmute()
    }

    /// Transmutes this build reference from BuildRef<T> to BuildRef<C>,
    ///
    pub fn transmute<C: Component>(self) -> BuildRef<'a, C, true> {
        BuildRef {
            compiler: self.compiler,
            entity: self.entity,
            _u: None,
            error: self.error,
        }
    }

    /// Returns self with async api disabled,
    ///
    pub fn disable_async(self) -> BuildRef<'a, T> {
        BuildRef {
            compiler: self.compiler,
            entity: self.entity,
            _u: None,
            error: self.error,
        }
    }
}

impl<T, const ENABLE_ASYNC: bool> From<Error> for BuildRef<'_, T, ENABLE_ASYNC> {
    fn from(value: Error) -> Self {
        Self { compiler: None, entity: None, error: Some(Arc::new(value)), _u: None }
    }
}