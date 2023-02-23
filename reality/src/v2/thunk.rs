use std::sync::Arc;

use async_trait::async_trait;
use specs::VecStorage;
use specs::Component;
use crate::Error;
use super::Build;
use super::Call;
use super::Properties;

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

/// Creates a thunk call from a type that implements call,
/// 
pub fn thunk_call(call: impl Call + 'static) -> ThunkCall {
    Thunk(Arc::new(call))
}

/// Creates a thunk build from a type that implements build,
/// 
pub fn thunk_build(build: impl Build + 'static) -> ThunkBuild {
    Thunk(Arc::new(build))
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
