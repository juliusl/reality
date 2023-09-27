use super::prelude::*;

/// Trait generalizing a storage target that can be used to initialize and store application resources,
///
pub trait StorageTarget {
    /// Value container type,
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

    /// Storage target type for handling namespaces,
    ///
    type Namespace: StorageTarget + Send + Sync + 'static;

    /// Creates a new StorageTarget namespace,
    ///
    /// Returns None if the namespace is no-longer available (reserved) or if the storage target does not support creating `Namespace` a namespace,
    ///
    /// **Note**: To reserve a namespace, it must be put into this storage target as a resource.
    ///
    /// That implies only `Namespace` types that are `Send + Sync` can be reserved in such a manner.
    ///
    #[allow(unused_variables)] // Optional trait fn
    fn create_namespace(
        &self,
        namespace: impl Into<String>,
        resource_key: Option<ResourceKey<Self::Namespace>>,
    ) -> Option<Self::Namespace> {
        None
    }

    /// Put a resource in storage w/ key
    /// 
    fn put_resource_at<T: Send + Sync + 'static>(&mut self, key: ResourceKey<T>, resource: T) {
        // encode ident to a resource_id
        // store addr as a key,
    }

    /// Put a resource in storage,
    ///
    /// Will always override the existing value,
    ///
    fn put_resource<T: Send + Sync + 'static>(&mut self, resource: T, resource_key: Option<ResourceKey<T>>);

    /// Take a resource from the storage target casting it back to it's original type,
    ///
    fn take_resource<T: Send + Sync + 'static>(&mut self, resource_key: Option<ResourceKey<T>>) -> Option<T>;

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
    fn key<T: Send + Sync + 'static>(
        resource_key: Option<ResourceKey<T>>) -> u64
    where
        Self: Sized,
    {
        use std::hash::Hash;

        let type_id = std::any::TypeId::of::<T>();
        let mut hasher = std::collections::hash_map::DefaultHasher::default();
        type_id.hash(&mut hasher);
        size_of::<T>().hash(&mut hasher);

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
    fn lazy_initialize_resource<T: Default + Send + Sync + 'static>(&self, resource_key: Option<ResourceKey<T>>)
    where
        Self: 'static,
    {
        self.lazy_put_resource(T::default(), resource_key)
    }

    /// Lazily puts a resource into the storage target
    /// 
    fn lazy_put_resource<T: Send + Sync + 'static>(&self, resource: T, resource_key: Option<ResourceKey<T>>)
    where
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
        AsyncStorageTarget(Arc::new(RwLock::new(self)))
    }
}
