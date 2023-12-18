use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::sync::RwLockMappedWriteGuard;
use tokio::sync::RwLockReadGuard;
use tokio::sync::RwLockWriteGuard;

use crate::prelude::*;

use super::target::StorageTargetKey;

// /// Struct containing a handle to a shared resource pointer,
// ///
// #[derive(Clone)]
// pub struct ResourceCell {
//     inner: Arc<RwLock<Box<dyn Send + Sync + 'static>>>,
// }

// impl ResourceCell {
//     /// Creates a new resource cell,
//     ///
//     pub fn new<T: Send + Sync + 'static>(resource: T) -> ResourceCell {
//         ResourceCell {
//             inner: Arc::new(RwLock::new(Box::new(resource))),
//         }
//     }
// }

/// Shared thread-safe storage target using Arc and tokio::RwLock,
///
#[derive(Clone, Default)]
pub struct Shared {
    /// Thread-safe resources,
    ///
    resources: HashMap<u64, Arc<RwLock<Box<dyn Send + Sync + 'static>>>>,
}

impl StorageTarget for Shared {
    type BorrowResource<'a, T: Send + Sync + 'static> = RwLockReadGuard<'a, T>;

    type BorrowMutResource<'a, T: Send + Sync + 'static> = RwLockMappedWriteGuard<'a, T>;

    type Namespace = Shared;

    fn create_namespace() -> Self::Namespace {
        Shared::default()
    }

    fn remove_resource_at(&mut self, key: ResourceKey<crate::Attribute>) -> bool {
        self.resources.remove(&key.key()).is_some()
    }

    fn maybe_put_resource<T: Send + Sync + 'static>(
        &mut self,
        resource: T,
        resource_key: StorageTargetKey<T>,
    ) -> Self::BorrowMutResource<'_, T> {
        let key = Self::key::<T>(resource_key);

        if !self.resources.contains_key(&key) {
            self.put_resource(resource, resource_key);
        }

        self.resource_mut(resource_key).expect("should exist")
    }

    fn contains<T: Send + Sync + 'static>(&self, resource_key: StorageTargetKey<T>) -> bool {
        let key = Self::key::<T>(resource_key);

        self.resources.contains_key(&key)
    }

    fn put_resource<T: Send + Sync + 'static>(
        &mut self,
        resource: T,
        resource_key: StorageTargetKey<T>,
    ) {
        let key = Self::key::<T>(resource_key);

        self.resources
            .insert(key, Arc::new(RwLock::new(Box::new(resource))));
    }

    fn take_resource<T: Send + Sync + 'static>(
        &mut self,
        resource_key: StorageTargetKey<T>,
    ) -> Option<Box<T>> {
        let key = Self::key::<T>(resource_key);
        let resource = self.resources.remove(&key);

        match resource {
            Some(r) => match Arc::try_unwrap(r) {
                Ok(r) => {
                    let inner = r.into_inner();
                    let inner = Box::leak(inner);

                    let r = from_ref_mut(inner);

                    // Making a backup of the pointer in case the cast fails
                    let backup = r;
                    let r = r.cast::<T>();

                    let _r = unsafe { Box::from_raw(r) };

                    // SAFETY: Check that the pointer being returned can successfully be returned
                    if unsafe { r.as_ref().is_none() } {
                        // Note: This code path should technically be impossible since the cast type is associated to the key itself
                        // However, in case there is an edge case that exists, this defends against a possible memory leak
                        let restoring = Arc::new(RwLock::new(unsafe { Box::from_raw(backup) }));
                        self.resources.insert(key, restoring);
                        None
                    } else {
                        Some(_r)
                    }
                }
                Err(l) => {
                    self.resources.insert(key, l);
                    None
                }
            },
            None => None,
        }
    }

    fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a self,
        resource_key: StorageTargetKey<T>,
    ) -> Option<Self::BorrowResource<'b, T>> {
        let key = Self::key::<T>(resource_key);

        if let Some(resource) = self.resources.get(&key) {
            resource.try_read().ok().and_then(|r| {
                match RwLockReadGuard::try_map(r, |v| {
                    let ptr = from_ref(v.as_ref()) as *const T;

                    unsafe { ptr.cast::<T>().as_ref() }
                }) {
                    Ok(g) => Some(g),
                    Err(_) => None,
                }
            })
        } else {
            None
        }
    }

    fn resource_mut<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a mut self,
        resource_key: StorageTargetKey<T>,
    ) -> Option<Self::BorrowMutResource<'b, T>> {
        let key = Self::key::<T>(resource_key);

        if let Some(resource) = self.resources.get(&key) {
            resource.try_write().ok().and_then(|r| {
                match RwLockWriteGuard::try_map(r, |v| {
                    let derefed: &mut (dyn Send + Sync) = v.deref_mut();
                    let ptr = from_ref_mut(derefed) as *mut T;

                    unsafe { ptr.as_mut() }
                }) {
                    Ok(g) => Some(g),
                    Err(_) => None,
                }
            })
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.resources.len()
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
async fn test_complex() {
    let mut shared = Shared::default();
    let resource_key = ResourceKey::with_hash("hello-complex");
    shared.put_resource(0u64, resource_key);

    borrow_mut!(shared, u64, "hello-complex", |r| => {
        *r += 2;
    });

    borrow!(shared, u64, "hello-complex", |r| => {
        println!("{r}");
    });

    borrow_mut!(shared, u64, "hello-complex", |r| => {
        *r += 2;
    });

    let mut _r = 0u64;
    borrow!(shared, u64, "hello-complex", |r| => {
        println!("{r}");
        _r = *r;
    });

    assert_eq!(4, _r);

    ()
}

#[tokio::test]
async fn test_async_dispatcher() {
    let shared = Shared::default().into_thread_safe();
    // Test initalizing a resource dispatcher and queueing dispatches
    let mut dispatcher = shared
        .maybe_intialize_dispatcher::<(u64, u64)>(ResourceKey::new())
        .await;
    dispatcher.queue_dispatch_mut(|(a, b)| {
        *a += 1;
        *b += 2;
    });

    dispatcher.queue_dispatch(|(a, b)| {
        println!("checking previous dispatch_mut");
        assert_eq!((4u64, 3u64), (*a, *b));
    });

    task!(dispatcher |(a, b)| => {
        let a = *a;
        let b = *b;
        println!("checking previous dispatch_mut_task");
        assert_eq!((4u64, 3u64), (a, b));
        async move { tokio::time::sleep(std::time::Duration::from_millis(a + b)).await; }
    });

    // Note that since this is a dispatch_mut, it will be executed before any non-mut dispatches, even though they are
    // queued prior to this dispatch.
    task_mut!(dispatcher |tuple| => {
        tuple.0 += 3;
        tuple.1 += 1;
        let a = tuple.0;
        let b = tuple.1;
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(a + b)).await;
        }
    });

    // Test dispatch draining
    dispatcher.dispatch_all().await;

    // Test that the queued dispatch executed
    borrow!(async shared (u64, u64), |res| => {
        assert_eq!((4, 3), *res);
    });

    assert_eq!(Some(Box::new((4, 3))), take!(async shared, (u64, u64)));

    // Test that the resource was removed
    borrow!(async shared (u64, u64), |_res| => {
        assert!(false, "should not be called");
    });

    ()
}
