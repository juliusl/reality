use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::HashMap;
use std::ops::Deref;
use std::ops::DerefMut;

use crate::Complex;
use crate::StorageTarget;
use super::prelude::*;

/// Simple storage target implementation,
///
#[derive(Default)]
pub struct Simple {
    /// Map of resources,
    ///
    resources: HashMap<u64, RefCell<Box<dyn Send + Sync + 'static>>>,
}

impl Simple {
    /// Creates a new empty simple storage target w/ dispatching enabled,
    ///
    pub fn new() -> Self {
        let mut simple = Self::default();
        simple.enable_dispatching();
        simple
    }
}

impl StorageTarget for Simple {
    type BorrowResource<'a, T: Send + Sync + 'static> = Ref<'a, T>;

    type BorrowMutResource<'a, T: Send + Sync + 'static> = RefMut<'a, T>;
    
    cfg_async_dispatcher! {
        type Namespace = Complex;
    }

    cfg_not_async_dispatcher! {
        type Namespace = Self;
    }

    fn create_namespace(
        &self, 
        namespace: impl Into<String>, 
        resource_key: Option<ResourceKey<Self::Namespace>>
    ) -> Option<Self::Namespace> {
        #[cfg(feature="async_dispatcher")]
        if self.resource::<Self::Namespace>(resource_key).is_some() {
            // If the consumer of StorageTarget can and is reserving namespaces, than do not 
            // return a namespace if one already exists
            return None;
        } 

        let mut ns = Self::Namespace::default();
        ns.put_resource(namespace.into(), None);

        Some(ns)
    }
    
    fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a self,
        resource_key: Option<ResourceKey<T>>
    ) -> Option<Self::BorrowResource<'b, T>> {
        let key = Self::key::<T>(resource_key);

        if let Some(resource) = self.resources.get(&key) {
            Ref::filter_map(resource.borrow(), |r| {
                let derefed: &(dyn Send + Sync) = r.deref();
                let ptr = from_ref(derefed) as *const T;

                if !ptr.is_null() {
                    unsafe { ptr.cast::<T>().as_ref() }
                } else {
                    None
                }
            })
            .ok()
        } else {
            None
        }
    }

    fn resource_mut<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a mut self,
        resource_key: Option<ResourceKey<T>>
    ) -> Option<Self::BorrowMutResource<'b, T>> {
        let key = Self::key::<T>(resource_key);

        if let Some(resource) = self.resources.get(&key) {
            RefMut::filter_map(resource.borrow_mut(), |r| {
                let derefed: &mut (dyn Send + Sync) = r.deref_mut();
                let ptr = from_ref_mut(derefed) as *mut T;

                if !ptr.is_null() { 
                    unsafe { ptr.as_mut() }
                } else {
                    None
                }
            })
            .ok()
        } else {
            None
        }
    }

    fn put_resource<T: Send + Sync + 'static>(
        &mut self, 
        resource: T,
        resource_key: Option<ResourceKey<T>>
    ) {
        let key = Self::key::<T>(resource_key);
        self.resources.insert(key, RefCell::new(Box::new(resource)));
    }

    fn take_resource<T: Send + Sync + 'static>(
        &mut self,
        resource_key: Option<ResourceKey<T>>
    ) -> Option<Box<T>> {
        let key = Self::key::<T>(resource_key);
        let resource = self.resources.remove(&key);

        resource.and_then(|r| {
            // This ensures after conversion, Box<T> can be properly dropped
            let t = r.into_inner();
            let t = Box::leak(t);
            let r = from_ref_mut(t);
            let r = r.cast::<T>();
            if !r.is_null() {
                Some(unsafe { Box::from_raw(r) })
            } else {
                None
            }
        })
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

#[tokio::test]
async fn test_simple_storage_target_resource_store() {
    let test_resource: Vec<u32> = vec![0, 1, 2, 3];

    let mut simple = Simple::new();
    simple.put_resource(test_resource, None);

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

    // Test reading resource after mutating
    {
        let resource = simple.resource::<Vec<u32>>(None);
        if let Some(resource) = resource {
            assert_eq!(vec![0, 1, 2, 3, 5, 6], resource[..]);
        }
    }

    // Test dispatch system
    let fun = |s: &mut Simple| {
        s.resource_mut::<u64>(None).map(|mut r| *r += 1);

        s.lazy_dispatch(|s| {
            let res = s.resource::<u64>(None);
            println!("dispatched after inc -- {:?}", res);
        });

        s.lazy_dispatch_mut(|s: &mut Simple| {
            s.resource_mut::<u64>(None).map(|mut r| *r += 1);

            s.lazy_dispatch(|s| {
                let res = s.resource::<u64>(None);
                println!("dispatched after inc -- {:?}", res);
            });
        });
    };

    // Test initialzing and updating a resource
    {
        simple.lazy_initialize_resource::<u64>(None);
        simple.lazy_dispatch_mut(fun);
        simple.lazy_dispatch_mut(fun);
        simple.lazy_dispatch_mut(fun);
        simple.lazy_dispatch_mut(fun);
    }
    simple.drain_dispatch_queues();

    // Check the result
    {
        let res = simple.resource::<u64>(None);
        assert_eq!(Some(4), res.as_deref().copied());
    }
    simple.drain_dispatch_queues();
}