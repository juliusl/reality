use std::sync::Arc;

use reality_derive::*;
use specs::world::LazyBuilder;
use specs::{Entity, Builder};

use crate::v2::Properties;
use crate::Result;

use super::Dispatch;

internal_use!();

thunk! {
/// Trait to build components for an entity,
///
pub trait Build
{
    /// Builds an entity w/ a lazy builder
    ///
    fn build(&self, lazy_builder: LazyBuilder) -> Result<Entity>;
}
}

impl<T: Fn(LazyBuilder) -> Result<Entity> + Sync + Send + 'static> Build for T {
    fn build(&self, lazy_builder: LazyBuilder) -> Result<Entity> {
        self(lazy_builder)
    }
}

impl<B: Build + Send + Sync> Dispatch for Arc<B> {
    fn dispatch<'a>(
        &self,
        dispatch_ref: crate::v2::DispatchRef<'a, crate::v2::Properties>,
    ) -> super::DispatchResult<'a> {
        dispatch_ref
            .transmute::<ThunkBuild>()
            .fork_into_with::<Properties>(|build, prop, lazy| {
                build.build(lazy.with(prop.clone()))
            })
    }
}
