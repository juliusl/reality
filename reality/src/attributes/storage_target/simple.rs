use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::ops::DerefMut;

use crate::{Attribute, StorageTarget};

/// Simple storage target,
///
#[derive(Default)]
pub struct Simple {
    /// Map of resources,
    ///
    resources: HashMap<u64, RefCell<Box<dyn Send + Sync + 'static>>>,
}

impl StorageTarget for Simple {
    type Attribute = Attribute;

    type BorrowResource<'a, T: Send + Sync + 'static> = Ref<'a, T>;

    type BorrowMutResource<'a, T: Send + Sync + 'static> = RefMut<'a, T>;

    fn entity(&self, id: <Self::Attribute as crate::attributes::Container>::Id) -> u64 {
        todo!()
    }

    fn create_entity(&self) -> <Self::Attribute as crate::attributes::Container>::Id {
        let entity = self.resource::<<Self::Attribute as crate::attributes::Container>::Id>(None);

        todo!()
    }

    fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a self,
        resource_id: Option<u64>,
    ) -> Option<Self::BorrowResource<'b, T>> {
        let type_id = std::any::TypeId::of::<T>();
        let mut hasher = DefaultHasher::new();
        type_id.hash(&mut hasher);

        if let Some(resource_id) = resource_id {
            resource_id.hash(&mut hasher);
        }

        if let Some(resource) = self.resources.get(&hasher.finish()) {
            Ref::filter_map(resource.borrow(), |r| {
                let derefed: &(dyn Send + Sync) = r.deref();
                let ptr = from_ref(derefed) as *const T;

                unsafe { ptr.cast::<T>().as_ref() }
            })
            .ok()
        } else {
            None
        }
    }

    fn resource_mut<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a mut self,
        resource_id: Option<u64>,
    ) -> Option<Self::BorrowMutResource<'b, T>> {
        let type_id = std::any::TypeId::of::<T>();
        let mut hasher = DefaultHasher::new();
        type_id.hash(&mut hasher);

        if let Some(resource_id) = resource_id {
            resource_id.hash(&mut hasher);
        }

        if let Some(resource) = self.resources.get(&hasher.finish()) {
            RefMut::filter_map(resource.borrow_mut(), |r| {
                let derefed: &mut (dyn Send + Sync) = r.deref_mut();
                let ptr = from_ref_mut(derefed) as *mut T;

                unsafe { ptr.as_mut() }
            })
            .ok()
        } else {
            None
        }
    }

    fn lazy_dispatch(&self, exec: impl FnOnce(&Self) + 'static + Send + Sync) {
        todo!()
    }

    fn lazy_dispatch_mut(&self, exec: impl FnOnce(&mut Self) + 'static + Send + Sync) {
        todo!()
    }

    fn put_resource<T: Send + Sync + 'static>(
        mut self,
        resource: T,
        resource_id: Option<u64>,
    ) -> Self {
        let type_id = std::any::TypeId::of::<T>();
        let mut hasher = DefaultHasher::new();
        type_id.hash(&mut hasher);

        if let Some(resource_id) = resource_id {
            resource_id.hash(&mut hasher);
        }

        self.resources
            .insert(hasher.finish(), RefCell::new(Box::new(resource)));
        self
    }
}

/// Convert a borrow to a raw pointer,
///
const fn from_ref<T: ?Sized>(r: &T) -> *const T {
    r
}

/// Convert a mut borrow to a raw mut pointer
///
fn from_ref_mut<T: ?Sized>(r: &mut T) -> *mut T {
    r
}

#[test]
fn test_simple_storage_target_resource_store() {
    let test_resource: Vec<u32> = vec![0, 1, 2, 3];
    println!("{:?}", test_resource);
    let mut simple = Simple::default().put_resource(test_resource, None);

    // Test inserting and mutating a resource
    {
        let resource = simple.resource::<Vec<u32>>(None);
        if let Some(ref resource) = resource {
            assert_eq!(vec![0, 1, 2, 3], resource[..]);
        }
        drop(resource);

        let resource = simple.resource_mut::<Vec<u32>>(None);
        if let Some(mut resource) = resource {
            resource.push(5);
            resource.push(6);
        }
    }

    // Test
    {
        let resource = simple.resource::<Vec<u32>>(None);
        if let Some(resource) = resource {
            assert_eq!(vec![0, 1, 2, 3, 5, 6], resource[..]);
        }
    }
}
