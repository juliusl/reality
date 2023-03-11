use std::sync::Arc;
use std::ops::Deref;
use std::fmt::Debug;
use specs::WorldExt;
use specs::LazyUpdate;
use specs::Entity;
use specs::Component;
use tracing::error;
use tracing::debug;
use crate::Error;

/// Trait to setup an update for an entity,
///
pub trait Update
where
    Self: Send + Sync,
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

/// Super-trait for Components to auto-register and insert an updated component to world storage,
/// 
pub trait AutoUpdateComponent: Update + Clone + Component + Debug {}

impl<T: AutoUpdateComponent> Update for T
where
    <Self as Component>::Storage: Default, 
{
    fn update(&self, updating: Entity, lazy_update: &LazyUpdate) -> Result<(), Error> {
        let next = self.clone();
        lazy_update.exec_mut(move |w| {
            w.register::<T>();
            match w.write_component::<T>().insert(updating, next) {
                Ok(last) => {
                    last.map(|l| debug!("Component updated, last: {:?}", l));
                }
                Err(err) => error!("Error inserting component, {err}"),
            }
        });

        Ok(())
    }
}
