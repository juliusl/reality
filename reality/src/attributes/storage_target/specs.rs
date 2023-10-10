use specs::WorldExt;
use specs::World;
use specs::LazyUpdate;
use specs::shred::ResourceId;
use specs::shred::FetchMut;
use specs::shred::Fetch;

use super::prelude::*;

impl StorageTarget for World {
    type BorrowResource<'a, T: Send + Sync + 'static> = Fetch<'a, T>;
    
    type BorrowMutResource<'a, T: Send + Sync + 'static> = FetchMut<'a, T>;

    type Namespace = World;

    fn create_namespace() -> Self::Namespace {
        World::new()
    }

    fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a self,
        config: ResourceStorageConfig<T>,
    ) -> Option<Self::BorrowResource<'b, T>> {
        if let Some(variance) = config.variance() {
            self.try_fetch_by_id::<T>(ResourceId::new_with_dynamic_id::<T>(variance))
        } else {
            self.try_fetch::<T>()
        }
    }

    fn resource_mut<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a mut self,
        config: ResourceStorageConfig<T>,
    ) -> Option<Self::BorrowMutResource<'b, T>> {
        if let Some(variance) = config.variance() {
            self.try_fetch_mut_by_id(ResourceId::new_with_dynamic_id::<T>(variance))
        } else {
            self.try_fetch_mut()
        }
    }

    fn put_resource<T: Send + Sync + 'static>(
        &mut self, 
        resource: T, 
        config: ResourceStorageConfig<T>,
    ) {
        if let Some(variance) = config.variance() {
            let resource_id = ResourceId::new_with_dynamic_id::<T>(variance);
            self.insert_by_id(resource_id, resource);
        } else {
            self.insert(resource);
        }
    }

    fn enable_dispatching(&mut self) where Self: 'static {
        // No-op
    }
    
    fn lazy_dispatch<F: FnOnce(&Self) + 'static + Send + Sync>(&self, exec: F){
        let lazy_update = self.read_resource::<LazyUpdate>();
        lazy_update.exec(|world| exec(world));
    }

    fn lazy_dispatch_mut<F: FnOnce(&mut Self) + 'static + Send + Sync>(&self, exec: F) {
        let lazy_update = self.read_resource::<LazyUpdate>();
        lazy_update.exec_mut(exec);
    }

    fn drain_dispatch_queues(&mut self) where Self: 'static {
        self.maintain();
    }

    fn take_resource<T: Send + Sync + 'static>(
        &mut self, 
        config: ResourceStorageConfig<T>,
    ) -> Option<Box<T>> {
        if let Some(variance) = config.variance() {
            let resource_id = ResourceId::new_with_dynamic_id::<T>(variance);
            self.remove_by_id::<T>(resource_id).map(Box::new)
        } else {
            self.remove::<T>().map(Box::new)
        }
    }
}