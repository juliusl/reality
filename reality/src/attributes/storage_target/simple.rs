use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::collections::HashMap;
use std::ops::Deref;
use std::ops::DerefMut;

use crate::Complex;
use crate::StorageTarget;
use crate::Attribute;
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
    type Attribute = Attribute;

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

                unsafe { ptr.cast::<T>().as_ref() }
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

                unsafe { ptr.as_mut() }
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
    ) -> Option<T> {
        let key = Self::key::<T>(resource_key);
        let resource = self.resources.remove(&key);

        resource.map(|r| {
            let t = r.into_inner();

            let r = from_ref(t.deref());

            unsafe { std::ptr::read(r.cast::<T>()) }
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

#[cfg(feature = "async_dispatcher")]
#[tokio::test]
async fn test_simple_async_dispatcher() {
    let name = std::any::type_name::<(u64, u64, i64, String, Simple)>();
    println!("{}", name
        .replace(", ", "__")
        .trim_start_matches(&['('])
        .trim_end_matches(&[')'])
        .replace("::", ".")
        .to_lowercase()
    );

    let simple = Complex::default().into_thread_safe();

    // Test initalizing a resource dispatcher and queueing dispatches
    let mut dispatcher = simple.intialize_dispatcher::<(u64, u64)>(None).await;
    dispatcher
        .queue_dispatch_mut(|(a, b)| {
            *a += 1;
            *b += 2;
        });

    dispatcher
        .queue_dispatch(|(a, b)| {
            println!("checking previous dispatch_mut");
            assert_eq!((1u64, 2u64), (*a, *b));
        });

    // Test dispatch draining
    dispatcher.dispatch_all().await;

    // Test that the queued dispatch executed
    {
        let res = simple.storage.read().await;
        let res = res.resource::<(u64, u64)>(None);
        assert_eq!(Some((1, 2)), res.as_deref().copied());
    }

    // Test that we can remove the resource
    {
        let mut res = simple.storage.write().await;
        let res = res.take_resource::<(u64, u64)>(None);
        assert_eq!(Some((1, 2)), res);
    }

    // Test that the resource was removed
    {
        let res = simple.storage.read().await;
        let res = res.resource::<(u64, u64)>(None);
        assert_eq!(None, res.as_deref().copied());
    }
}
