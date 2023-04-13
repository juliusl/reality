use crate::Result;
use specs::Entity;
use specs::LazyUpdate;
use std::ops::Deref;
use std::sync::Arc;

/// Trait to setup an update for an entity,
///
pub trait Update<T = ()>
where
    Self: Send + Sync,
{
    /// Updates an entity,
    ///
    fn update(&self, updating: Entity, lazy_update: &LazyUpdate) -> Result<()>;
}

impl Update for Arc<dyn Update> {
    fn update(&self, updating: Entity, lazy_update: &LazyUpdate) -> Result<()> {
        self.deref().update(updating, lazy_update)
    }
}

impl<T: Fn(Entity, &LazyUpdate) -> Result<()> + Send + Sync + 'static> Update for T {
    fn update(&self, updating: Entity, lazy_update: &LazyUpdate) -> Result<()> {
        self(updating, lazy_update)
    }
}
