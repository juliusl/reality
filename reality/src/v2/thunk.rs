use std::sync::Arc;

use async_trait::async_trait;
use specs::LazyUpdate;
use specs::VecStorage;
use specs::Component;
use crate::Error;
use super::Properties;

mod call;
pub use call::Call;

mod build;
pub use build::Build;

mod update;
pub use update::Update;
pub use update::AutoUpdateComponent;

mod listen;
pub use listen::Accept;
pub use listen::Listen;
pub use listen::Listener;
pub use listen::ERROR_NOT_ACCEPTED;

/// Wrapper struct Component for storing a reference to a dyn Trait reference to be called later,
/// 
/// Before the thunk is called, it will be cloned
/// 
#[derive(Default, Component, Clone)]
#[storage(VecStorage)]
pub struct Thunk<T: Send + Sync + 'static> {
    /// Thunk type,
    /// 
    thunk: T,
    /// Hint of the lifetime of this thunk,
    /// 
    /// Defaults to Once,
    /// 
    lifetime_hint: Lifetime,
}

impl<T: Send + Sync + 'static> Thunk<T> {
    /// Returns this thunk w/ lifetime,
    /// 
    pub fn with_lifetime(mut self, lifetime: Lifetime) -> Self {
        self.lifetime_hint = lifetime;
        self
    }

    /// Returns this thunk w/ an unlimited lifetime,
    /// 
    pub fn with_unlimited(mut self) -> Self {
        self.lifetime_hint = Lifetime::Unlimited;
        self
    }
}

/// Enumeration of lifetime options for a thunk component,
/// 
#[derive(Default, Copy, Clone)]
pub enum Lifetime {
    /// (Default) This thunk should only be executed once,
    /// 
    #[default]
    Once,
    /// This thunk can be executed an unlimited amount of times,
    /// 
    Unlimited,
}

/// Type-alias for a thunk call component,
/// 
pub type ThunkCall = Thunk<Arc<dyn Call>>;

/// Type-alias for a thunk build component,
/// 
pub type ThunkBuild = Thunk<Arc<dyn Build>>;

/// Type-alias for a thunk update component,
/// 
pub type ThunkUpdate = Thunk<Arc<dyn Update>>;

/// Type-alias for a thunk listen component,
/// 
pub type ThunkListen = Thunk<Arc<dyn Listen>>;

/// Creates a thunk call from a type that implements Call,
/// 
pub fn thunk_call(call: impl Call + 'static) -> ThunkCall {
    Thunk { thunk: Arc::new(call), lifetime_hint: Default::default() }
}

/// Creates a thunk build from a type that implements Build,
/// 
pub fn thunk_build(build: impl Build + 'static) -> ThunkBuild {
    Thunk { thunk: Arc::new(build), lifetime_hint: Default::default() }
}

/// Creates a thunk update from a type that implements Update,
/// 
pub fn thunk_update(update: impl Update + 'static) -> ThunkUpdate {
    Thunk { thunk: Arc::new(update), lifetime_hint: Default::default() }
}

/// Creates a thunk listen from a type that implements Listen,
/// 
pub fn thunk_listen(listen: impl Listen + 'static) -> ThunkListen {
    Thunk { thunk: Arc::new(listen), lifetime_hint: Default::default() }
}

#[async_trait]
impl<T: Call + Send + Sync> Call for Thunk<T> {
    async fn call(&self) -> Result<Properties, Error> {
        self.thunk.call().await
    }
}

#[async_trait]
impl<T: Listen + Send + Sync> Listen for Thunk<T> {
    async fn listen(&self, properties: Properties, lazy_update: &LazyUpdate) -> Result<(), Error> {
        self.thunk.listen(properties, lazy_update).await
    }
}

impl<T: Build + Send + Sync> Build for Thunk<T> {
    fn build(&self, lazy_builder: specs::world::LazyBuilder) -> Result<specs::Entity, Error> {
        self.thunk.build(lazy_builder)
    }
}

impl<T: Update + Send + Sync> Update for Thunk<T> {
    fn update(&self, updating: specs::Entity, lazy_update: &specs::LazyUpdate) -> Result<(), Error> {
        self.thunk.update(updating, lazy_update)
    }
}
