use std::{fmt::Debug, ops::{Deref, DerefMut}};

use super::Container;

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

pub mod spec_storage_target {
    use specs::WorldExt;
    use specs::World;
    use specs::LazyUpdate;
    use specs::shred::ResourceId;
    use specs::shred::FetchMut;
    use specs::shred::Fetch;
    use crate::Attribute;
    use crate::attributes::Container;

    use super::StorageTarget;

    impl StorageTarget for World {
        type Attribute = Attribute;

        type BorrowResource<'a, T: Send + Sync + 'static> = Fetch<'a, T>;
        
        type BorrowMutResource<'a, T: Send + Sync + 'static> = FetchMut<'a, T>;

        fn entity(&self, id: <Self::Attribute as Container>::Id) -> u64 {
            self.entities().entity(id).id() as u64
        }

        fn create_entity(&self) -> <Self::Attribute as Container>::Id {
            self.entities().create().id() as u32
        }

        fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
            &'a self,
            resource_id: Option<u64>,
        ) -> Option<Self::BorrowResource<'b, T>> {
            if let Some(resource_id) = resource_id {
                self.try_fetch_by_id::<T>(ResourceId::new_with_dynamic_id::<T>(resource_id))
            } else {
                self.try_fetch::<T>()
            }
        }

        fn resource_mut<'a: 'b, 'b, T: Send + Sync + 'static>(
            &'a mut self,
            resource_id: Option<u64>,
        ) -> Option<Self::BorrowMutResource<'b, T>> {
            if let Some(resource_id) = resource_id {
                self.try_fetch_mut_by_id(ResourceId::new_with_dynamic_id::<T>(resource_id))
            } else {
                self.try_fetch_mut()
            }
        }

        fn lazy_dispatch(&self, exec: impl FnOnce(&Self) + 'static + Send + Sync) {
            let lazy_update = self.read_resource::<LazyUpdate>();
            lazy_update.exec(|world| exec(world));
        }

        fn lazy_dispatch_mut(
            &self,
            exec: impl FnOnce(&mut Self) + 'static + Send + Sync,
        ) {
            let lazy_update = self.read_resource::<LazyUpdate>();
            lazy_update.exec_mut(exec);
        }
    }
}
