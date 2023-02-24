use std::sync::Arc;

use async_trait::async_trait;
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

/// Wrapper struct Component for storing a reference to a dyn Trait reference to be called later,
/// 
/// Before the thunk is called, it will be cloned
/// 
#[derive(Default, Component, Clone)]
#[storage(VecStorage)]
pub struct Thunk<T: Send + Sync + 'static>(T);

/// Type-alias for a thunk call component,
/// 
pub type ThunkCall = Thunk<Arc<dyn Call>>;

/// Type-alias for a thunk build component,
/// 
pub type ThunkBuild = Thunk<Arc<dyn Build>>;

/// Type-alias for a thunk update component,
/// 
pub type ThunkUpdate = Thunk<Arc<dyn Update>>;

/// Creates a thunk call from a type that implements Call,
/// 
pub fn thunk_call(call: impl Call + 'static) -> ThunkCall {
    Thunk(Arc::new(call))
}

/// Creates a thunk build from a type that implements Build,
/// 
pub fn thunk_build(build: impl Build + 'static) -> ThunkBuild {
    Thunk(Arc::new(build))
}

/// Creates a thunk update from a type that implements Update,
/// 
pub fn thunk_update(update: impl Update + 'static) -> ThunkUpdate {
    Thunk(Arc::new(update))
}

#[async_trait]
impl<T: Call + Send + Sync> Call for Thunk<T> {
    async fn call(&self) -> Result<Properties, Error> {
        self.0.call().await
    }
}

impl<T: Build + Send + Sync> Build for Thunk<T> {
    fn build(&self, lazy_builder: specs::world::LazyBuilder) -> Result<specs::Entity, Error> {
        self.0.build(lazy_builder)
    }
}

impl<T: Update + Send + Sync> Update for Thunk<T> {
    fn update(&self, updating: specs::Entity, lazy_update: &specs::LazyUpdate) -> Result<(), Error> {
        self.0.update(updating, lazy_update)
    }
}
