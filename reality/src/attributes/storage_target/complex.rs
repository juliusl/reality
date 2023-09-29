use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::sync::RwLockMappedWriteGuard;
use tokio::sync::RwLockReadGuard;
use tokio::sync::RwLockWriteGuard;

use super::prelude::*;
use crate::StorageTarget;

/// Complex thread-safe storage target using Arc and tokio::RwLock,
///
#[derive(Default)]
pub struct Complex {
    /// Thread-safe resources,
    ///
    resources: HashMap<u64, Arc<RwLock<Box<dyn Send + Sync + 'static>>>>,
}

impl StorageTarget for Complex {
    type BorrowResource<'a, T: Send + Sync + 'static> = RwLockReadGuard<'a, T>;

    type BorrowMutResource<'a, T: Send + Sync + 'static> = RwLockMappedWriteGuard<'a, T>;

    type Namespace = Complex;

    fn put_resource<T: Send + Sync + 'static>(
        &mut self,
        resource: T,
        resource_key: Option<ResourceKey<T>>,
    ) {
        let key = Self::key::<T>(resource_key);

        self.resources
            .insert(key, Arc::new(RwLock::new(Box::new(resource))));
    }

    fn take_resource<T: Send + Sync + 'static>(
        &mut self,
        resource_key: Option<ResourceKey<T>>,
    ) -> Option<Box<T>> {
        let key = Self::key::<T>(resource_key);
        let resource = self.resources.remove(&key);

        match resource {
            Some(r) => match Arc::try_unwrap(r) {
                Ok(r) => {
                    let inner = r.into_inner();
                    let inner = Box::leak(inner);

                    let r = from_ref_mut(inner);

                    let r = r.cast::<T>();

                    if r.is_null() {
                        None
                    } else {
                        Some(unsafe { Box::from_raw(r) })
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
        resource_key: Option<ResourceKey<T>>,
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
        resource_key: Option<ResourceKey<T>>,
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
    let mut complex = Complex::default();
    let resource_key = Some(ResourceKey::with_label("hello-complex"));
    complex.put_resource(0u64, resource_key);
    {
        let resource = complex.resource_mut::<u64>(resource_key);
        if let Some(mut r) = resource {
            *r += 2;
        }
    }

    {
        let resource = complex.resource::<u64>(resource_key);
        if let Some(r) = resource {
            println!("{r}");
        }
    }

    {
        let resource = complex.resource_mut::<u64>(resource_key);
        if let Some(mut r) = resource {
            *r += 2;
        }
    }

    {
        let resource = complex.resource::<u64>(resource_key);
        if let Some(r) = resource {
            println!("{r}");
        }

        const IDENT: &'static str = "test1234";

        let p = (IDENT.as_ptr() as u64, IDENT.len());

        let uuid = uuid::Uuid::from_fields(p.1 as u32, 0, 0, &p.0.to_be_bytes());
        println!("{}", uuid);
    }
}

#[tokio::test]
async fn test_async_dispatcher() {
    let name = std::any::type_name::<(u64, u64, i64, String, Simple)>();
    println!(
        "{}",
        name.replace(", ", "__")
            .trim_start_matches(&['('])
            .trim_end_matches(&[')'])
            .replace("::", ".")
            .to_lowercase()
    );

    let simple = Complex::default().into_thread_safe();

    // Test initalizing a resource dispatcher and queueing dispatches
    let mut dispatcher = simple.intialize_dispatcher::<(u64, u64)>(None).await;
    dispatcher.queue_dispatch_mut(|(a, b)| {
        *a += 1;
        *b += 2;
    });

    dispatcher.queue_dispatch(|(a, b)| {
        println!("checking previous dispatch_mut");
        assert_eq!((4u64, 3u64), (*a, *b));
    });

    dispatcher.queue_dispatch_task(task!(|(a, b)| => {
        let a = *a;
        let b = *b;
        println!("checking previous dispatch_mut_task");
        assert_eq!((4u64, 3u64), (a, b));
        async move { tokio::time::sleep(std::time::Duration::from_secs(a + b)).await; }
    }));

    // Note that since this is a dispatch_mut, it will be executed before any non-mut dispatches, even though they are 
    // queued prior to this dispatch.
    dispatcher.queue_dispatch_mut_task(task_mut!(|tuple| => {
        tuple.0 += 3;
        tuple.1 += 1;
        let a = tuple.0;
        let b = tuple.1;
        async move {
            tokio::time::sleep(std::time::Duration::from_secs(a + b)).await;
        }
    }));

    // Test dispatch draining
    dispatcher.dispatch_all().await;

    // Test that the queued dispatch executed
    {
        let res = simple.storage.read().await;
        let res = res.resource::<(u64, u64)>(None);
        assert_eq!(Some((4, 3)), res.as_deref().copied());
    }

    // Test that we can remove the resource
    {
        let mut res = simple.storage.write().await;
        let res = res.take_resource::<(u64, u64)>(None);
        assert_eq!(Some(&(4, 3)), res.as_deref());
    }

    // Test that the resource was removed
    {
        let res = simple.storage.read().await;
        let res = res.resource::<(u64, u64)>(None);
        assert_eq!(None, res.as_deref().copied());
    }
}
