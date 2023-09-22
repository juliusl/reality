use std::ops::DerefMut;
use std::ops::Deref;
use std::fmt::Debug;
use super::Container;

cfg_specs! {
    pub mod specs;
    pub use specs::*;
}

pub mod simple;
pub use simple::Simple;

/// Trait generalizing an Entity-Component-System storage backend,
/// 
pub trait StorageTarget {
    /// Attribute container type,
    /// 
    type Attribute: Container + Send + Sync + Clone + Debug;

    /// Container for borrowing a resource from the storage target,
    ///
    type BorrowResource<'a, T: Send + Sync + 'static>: Deref<Target = T>
    where
        Self: 'a;

    /// Container for mutably borrowing a resource from the storage target,
    ///
    type BorrowMutResource<'a, T: Send + Sync + 'static>: Deref<Target = T> + DerefMut<Target = T>
    where
        Self: 'a;

    /// Convert the id used by an attribute to a u64,
    /// 
    fn entity(&self, id: <Self::Attribute as Container>::Id) -> u64;

    /// Creates a new entity,
    /// 
    fn create_entity(&self) -> <Self::Attribute as Container>::Id;

    /// Put a resource in storage,
    /// 
    fn put_resource<T: Send + Sync + 'static>(self, resource: T, resource_id: Option<u64>) -> Self;

    /// Get read-access to a resource owned by the storage target,
    ///
    fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a self,
        resource_id: Option<u64>,
    ) -> Option<Self::BorrowResource<'b, T>>;

    /// Get read/write access to a resource owned by the storage target,
    ///
    fn resource_mut<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a mut self,
        resource_id: Option<u64>,
    ) -> Option<Self::BorrowMutResource<'b, T>>;

    /// Lazily dispatch a fn w/ borrowed access to inner storage,
    ///
    fn lazy_dispatch(&self, exec: impl FnOnce(&Self) + 'static + Send + Sync);

    /// Lazily dispatch a fn w/ mutable borrowed access to inner storage,
    ///
    fn lazy_dispatch_mut(&self, exec: impl FnOnce(&mut Self) + 'static + Send + Sync);
}
