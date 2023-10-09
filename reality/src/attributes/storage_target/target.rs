use super::prelude::*;
use crate::{Callback, CallbackMut, Handler};
use std::ops::Deref;
use std::ops::DerefMut;

/// Trait generalizing a storage target that can be used to initialize and store application resources,
///
pub trait StorageTarget {
    /// Container for borrowing a resource from the storage target,
    ///
    type BorrowResource<'a, T: Send + Sync + 'static>: Deref<Target = T> + Send + Sync
    where
        Self: 'a;

    /// Container for mutably borrowing a resource from the storage target,
    ///
    type BorrowMutResource<'a, T: Send + Sync + 'static>: Deref<Target = T>
        + DerefMut<Target = T>
        + Send
        + Sync
    where
        Self: 'a;

    /// Storage target type for handling namespaces,
    ///
    type Namespace: StorageTarget + Unpin + Send + Sync + 'static;

    /// Creates the storage target implementation for namespace support,
    ///
    fn create_namespace() -> Self::Namespace;

    /// Creates a shared namespace,
    ///
    /// Returns a thread safe storage target wrapper.
    ///
    #[cfg(feature = "async_dispatcher")]
    fn shared_namespace(
        &self,
        namespace: impl std::hash::Hash,
    ) -> AsyncStorageTarget<Self::Namespace>
    where
        Self: 'static,
    {
        let resource_key = ResourceKey::<AsyncStorageTarget<Self::Namespace>>::with_hash(namespace);

        if let Some(ns) = self.resource(Some(resource_key)) {
            ns.clone()
        } else {
            let mut ns = Self::create_namespace();
            ns.enable_dispatching();
            let shared = ns.into_thread_safe();
            self.lazy_put_resource(shared.clone(), Some(resource_key));
            shared
        }
    }

    /// Put a resource in storage w/ key
    ///
    fn put_resource_at<T: Send + Sync + 'static>(&mut self, _key: ResourceKey<T>, _resource: T) {
        // encode ident to a resource_id
        // store addr as a key,
    }

    /// Put a resource in storage,
    ///
    /// Will always override the existing value,
    ///
    fn put_resource<T: Send + Sync + 'static>(
        &mut self,
        resource: T,
        resource_key: Option<ResourceKey<T>>,
    );

    /// Take a resource from the storage target casting it back to it's original type,
    ///
    fn take_resource<T: Send + Sync + 'static>(
        &mut self,
        resource_key: Option<ResourceKey<T>>,
    ) -> Option<Box<T>>;

    /// Get read-access to a resource owned by the storage target,
    ///
    /// -- **Panics** --
    ///
    /// Will panic if the resource has a mutable borrow.
    ///
    /// --
    ///
    fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a self,
        resource_key: Option<ResourceKey<T>>,
    ) -> Option<Self::BorrowResource<'b, T>>;

    /// Get read/write access to a resource owned by the storage target,
    ///
    /// -- **Panics** --
    ///
    /// Will panic if the resource is already being borrowed.
    ///
    /// --
    ///
    fn resource_mut<'a: 'b, 'b, T: Send + Sync + 'static>(
        &'a mut self,
        resource_key: Option<ResourceKey<T>>,
    ) -> Option<Self::BorrowMutResource<'b, T>>;

    /// Returns a hashed key by Type and optional resource_id,
    ///
    fn key<T: Send + Sync + 'static>(resource_key: Option<ResourceKey<T>>) -> u64
    where
        Self: Sized,
    {
        use std::hash::Hash;
        use std::hash::Hasher;

        let type_id = std::any::TypeId::of::<T>();
        let mut hasher = std::collections::hash_map::DefaultHasher::default();
        type_id.hash(&mut hasher);
        std::mem::size_of::<T>().hash(&mut hasher);

        let mut key = hasher.finish();
        let _key = key;
        if let Some(resource_key) = resource_key {
            let resource_id = resource_key.key();
            key ^= resource_id;
            debug_assert_eq!(_key, key ^ resource_id);
        }

        key
    }

    /// Enables built-in dispatch queues,
    ///
    /// -- **Note** --
    ///
    /// If not called, then `lazy_dispatch`/`lazy_dispatch_mut`/`drain_default_dispatch_queues` will be no-ops,
    ///
    /// In this case `lazy_dispatch` and `lazy_dispatch_mut` must be overridden.
    ///
    /// --
    ///
    fn enable_dispatching(&mut self)
    where
        Self: 'static,
    {
        self.put_resource(DispatchQueue::<Self>::default(), None);
        self.put_resource(DispatchMutQueue::<Self>::default(), None);
    }

    /// Lazily initialize a resource that is `Default`,
    ///
    fn lazy_initialize_resource<T: Default + Send + Sync + 'static>(
        &self,
        resource_key: Option<ResourceKey<T>>,
    ) where
        Self: 'static,
    {
        self.lazy_put_resource(T::default(), resource_key)
    }

    /// Lazily puts a resource into the storage target
    ///
    fn lazy_put_resource<T: Send + Sync + 'static>(
        &self,
        resource: T,
        resource_key: Option<ResourceKey<T>>,
    ) where
        Self: 'static,
    {
        self.lazy_dispatch_mut(move |s| s.put_resource(resource, resource_key));
    }

    /// Lazily dispatch a fn w/ a reference to the storage target,
    ///
    fn lazy_dispatch<F: FnOnce(&Self) + 'static + Send + Sync>(&self, exec: F)
    where
        Self: 'static,
    {
        if let Some(queue) = self.resource::<DispatchQueue<Self>>(None) {
            if let Ok(mut queue) = queue.lock() {
                queue.push_back(Box::new(exec));
            }
        }
    }

    /// Lazily dispatch a fn w/ a mutable reference to the storage target,
    ///
    fn lazy_dispatch_mut<F: FnOnce(&mut Self) + 'static + Send + Sync>(&self, exec: F)
    where
        Self: 'static,
    {
        if let Some(queue) = self.resource::<DispatchMutQueue<Self>>(None) {
            if let Ok(mut queue) = queue.lock() {
                queue.push_back(Box::new(exec));
            }
        }
    }

    /// Drains all dispatch queues,
    ///
    /// ## Notes on Default implementation
    ///
    /// The default implementation will call the mutable dispatches first and then call the non-mutable dispatches after.
    ///
    /// In this case if `lazy_dispatch` is called inside of `lazy_dispatch_mut`, it will immediately be called after all mutable dispatches have completed.
    ///
    /// If `lazy_dispatch_mut` is called inside of `lazy_dispatch_mut`, then these dispatches will not be called until the next `drain_dispatch_queues`.
    ///
    /// If overriden, this behavior cannot be gurranteed.
    ///
    fn drain_dispatch_queues(&mut self)
    where
        Self: 'static,
    {
        let mut tocall = vec![];
        {
            let queue = self.resource_mut::<DispatchMutQueue<Self>>(None);
            if let Some(queue) = queue {
                if let Ok(mut queue) = queue.lock() {
                    while let Some(func) = queue.pop_front() {
                        tocall.push(func);
                    }
                }
            }
        }
        {
            for call in tocall.drain(..) {
                call(self);
            }
        }

        let mut tocall = vec![];
        {
            let queue = self.resource_mut::<DispatchQueue<Self>>(None);
            if let Some(queue) = queue {
                if let Ok(mut queue) = queue.lock() {
                    while let Some(func) = queue.pop_front() {
                        tocall.push(func);
                    }
                }
            }
        }
        {
            for call in tocall.drain(..) {
                call(self);
            }
        }
    }

    /// Consume the storage target returning a thread safe version,
    ///
    /// This enables individual dispatchers to be created for stored resources, and for a cloneable reference to the underlying storage target
    ///
    /// **Requires the `async_dispatcher` feature**
    ///
    #[cfg(feature = "async_dispatcher")]
    fn into_thread_safe(self) -> AsyncStorageTarget<Self>
    where
        Self: Sized,
    {
        use std::sync::Arc;

        AsyncStorageTarget {
            storage: Arc::new(RwLock::new(self)),
            runtime: Some(tokio::runtime::Handle::current()),
        }
    }

    /// Consume the storage target returning a thread safe version w/ specific runtime handle,
    ///
    /// This enables individual dispatchers to be created for stored resources, and for a cloneable reference to the underlying storage target
    ///
    /// **Requires the `async_dispatcher` feature**
    ///
    #[cfg(feature = "async_dispatcher")]
    fn into_thread_safe_with(self, handle: tokio::runtime::Handle) -> AsyncStorageTarget<Self>
    where
        Self: Sized,
    {
        use std::sync::Arc;

        AsyncStorageTarget {
            storage: Arc::new(RwLock::new(self)),
            runtime: Some(handle),
        }
    }

    /// Adds a callback as a resource,
    ///
    fn add_callback<Arg: Send + Sync + 'static, H: Handler<Self, Arg>>(
        &mut self,
        resource_key: Option<ResourceKey<Callback<Self, Arg>>>,
    ) where
        Self: Sized + 'static,
    {
        self.put_resource(Callback::new::<H>(), resource_key)
    }

    /// Lazily queues a dispatch for a callback w/ Arg,
    ///
    /// Returns true if a callback exists and was queued
    ///
    fn callback<Arg: Send + Sync + 'static>(
        &self,
        resource_key: Option<ResourceKey<Callback<Self, Arg>>>,
    ) -> Option<Callback<Self, Arg>>
    where
        Self: Sized + 'static,
    {
        self.resource(resource_key).map(|c| c.deref().clone())
    }

    /// Lazily queues a mutable dispatch for a callback w/ Arg if one exists,
    ///
    /// Returns true if a callback exists and was queued,
    ///
    fn callback_mut<Arg: Send + Sync + 'static>(
        &self,
        resource_key: Option<ResourceKey<CallbackMut<Self, Arg>>>,
    ) -> Option<CallbackMut<Self, Arg>>
    where
        Self: Sized + 'static,
    {
        self.resource(resource_key).map(|c| c.deref().clone())
    }

    /// Lazily queues a dispatch for a callback w/ Arg,
    ///
    /// Returns true if a callback exists and was queued
    ///
    fn lazy_callback<Arg: Send + Sync + 'static>(&self, callback: Callback<Self, Arg>, arg: Arg)
    where
        Self: Sized + 'static,
    {
        self.lazy_dispatch(move |s| callback.handle(s, arg))
    }

    /// Lazily queues a mutable dispatch for a callback w/ Arg if one exists,
    ///
    /// Returns true if a callback exists and was queued,
    ///
    fn lazy_callback_mut<Arg: Send + Sync + 'static>(
        &self,
        callback: CallbackMut<Self, Arg>,
        arg: Arg,
    ) where
        Self: Sized + 'static,
    {
        self.lazy_dispatch_mut(move |s| callback.handle(s, arg))
    }
}
