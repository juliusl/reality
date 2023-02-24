use std::{sync::Arc, ops::Deref};

use specs::{Entity, LazyUpdate};

use crate::Error;


/// Trait to setup an update for an entity,
/// 
pub trait Update
    where
        Self: Send + Sync 
{
    /// Updates an entity,
    /// 
    fn update(&self, updating: Entity, lazy_update: &LazyUpdate) -> Result<(), Error>;
}

impl Update for Arc<dyn Update> {
    fn update(&self, updating: Entity, lazy_update: &LazyUpdate) -> Result<(), Error> {
        self.deref().update(updating, lazy_update)
    }
}