use std::collections::HashMap;
use std::sync::Arc;
use std::ops::Deref;
use std::ops::DerefMut;

use tokio::sync::RwLockMappedWriteGuard;
use tokio::sync::RwLockReadGuard;
use tokio::sync::RwLock;
use tokio::sync::RwLockWriteGuard;

use crate::{StorageTarget, Attribute};
use super::prelude::*;

/// Complex thread-safe storage target using Arc and tokio::RwLock,
/// 
#[derive(Default)]
pub struct Complex {
    /// Thread-safe resources,
    /// 
    resources: HashMap<u64, Arc<RwLock<Box<dyn Send + Sync + 'static>>>>
}

impl StorageTarget for Complex {
    type Attribute = Attribute;

    type BorrowResource<'a, T: Send + Sync + 'static> = RwLockReadGuard<'a, T>;

    type BorrowMutResource<'a, T: Send + Sync + 'static> = RwLockMappedWriteGuard<'a, T>;

    type Namespace = Complex;

    fn put_resource<T: Send + Sync + 'static>(
        &mut self, 
        resource: T,
        resource_key: Option<ResourceKey>
    ) {
        let key = Self::key::<T>(resource_key);

        self.resources.insert(key, Arc::new(RwLock::new(Box::new(resource))));
    }

    fn take_resource<T: Send + Sync + 'static>(
        &mut self,
        resource_key: Option<ResourceKey>
    ) -> Option<T> {
        let key = Self::key::<T>(resource_key);
        let resource = self.resources.remove(&key);

        match resource {
            Some(r) =>  match Arc::try_unwrap(r) {
                Ok(r) => {
                    let inner = r.into_inner();

                    let r = from_ref(inner.deref());

                    let r = r.cast::<T>();

                    if r.is_null() {
                        None
                    } else if std::mem::size_of::<T>().is_power_of_two() {
                        Some(unsafe { std::ptr::read(r) })
                    } else {
                        Some(unsafe { std::ptr::read_unaligned(r) })
                    }
                },
                Err(l) => {
                    self.resources.insert(key, l);
                    None
                },
            },
            None => None,
        }
    }

    fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a self,
        resource_key: Option<ResourceKey>
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
        resource_key: Option<ResourceKey>
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
    complex.put_resource(0u64, None);
    {
        let resource = complex.resource_mut::<u64>(None);
        if let Some(mut r) = resource {
            *r += 2;
        }
    }

    {
        let resource = complex.resource::<u64>(None);
        if let Some(r) = resource {
            println!("{r}");
        }
    }

    {
        let resource = complex.resource_mut::<u64>(None);
        if let Some(mut r) = resource {
            *r += 2;
        }
    }

    {
        let resource = complex.resource::<u64>(None);
        if let Some(r) = resource {
            println!("{r}");
        }

        const IDENT: &'static str = "test1234";

        let p = (IDENT.as_ptr() as u64, IDENT.len());

        let uuid = uuid::Uuid::from_fields(p.1 as u32, 0, 0, &p.0.to_be_bytes());
        println!("{}", uuid);
    }
}