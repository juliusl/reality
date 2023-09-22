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

    fn lazy_dispatch<F: FnOnce(&Self) + 'static + Send + Sync>(&self, exec: F){
        let lazy_update = self.read_resource::<LazyUpdate>();
        lazy_update.exec(|world| exec(world));
    }

    fn lazy_dispatch_mut<F: FnOnce(&mut Self) + 'static + Send + Sync>(&self, exec: F) {
        let lazy_update = self.read_resource::<LazyUpdate>();
        lazy_update.exec_mut(exec);
    }

    fn put_resource<T: Send + Sync + 'static>(&mut self, resource: T, resource_id: Option<u64>) {
        if let Some(resource_id) = resource_id {
            let resource_id = ResourceId::new_with_dynamic_id::<T>(resource_id);
            self.insert_by_id(resource_id, resource);
        } else {
            self.insert(resource);
        }
    }
}