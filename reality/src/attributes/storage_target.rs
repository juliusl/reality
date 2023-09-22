use std::ops::DerefMut;
use std::ops::Deref;
use std::fmt::Debug;
use super::Container;

cfg_specs! {
    pub mod specs;
}

pub mod simple;
pub use simple::Simple;

/// Type-alias for a thread safe dispatch queue,
/// 
type DispatchQueue<S> = std::sync::Mutex<std::collections::VecDeque<Box<dyn FnOnce(&S) + 'static + Send + Sync>>>;

/// Type-alias for a thread safe dispatch-mut queue,
/// 
type DispatchMutQueue<S> = std::sync::Mutex<std::collections::VecDeque<Box<dyn FnOnce(&mut S) + 'static + Send + Sync>>>;


/// Trait generalizing an Entity-Component-System storage backend,
/// 
pub trait StorageTarget {
    /// Attribute container type,
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

    /// Convert the id used by an attribute to a u64,
    /// 
    fn entity(&self, id: <Self::Attribute as Container>::Id) -> u64;

    /// Creates a new entity,
    /// 
    fn create_entity(&self) -> <Self::Attribute as Container>::Id;

    /// Put a resource in storage,
    /// 
    fn put_resource<T: Send + Sync + 'static>(&mut self, resource: T, resource_id: Option<u64>);

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
        resource_id: Option<u64>,
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
        resource_id: Option<u64>,
    ) -> Option<Self::BorrowMutResource<'b, T>>;

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
    fn enable_dispatching(&mut self) where Self: 'static {
        self.put_resource(DispatchQueue::<Self>::default(), None);
        self.put_resource(DispatchMutQueue::<Self>::default(), None);
    }

    /// Lazily dispatch a fn w/ a reference to the storage target,
    /// 
    fn lazy_dispatch<F: FnOnce(&Self) + 'static + Send + Sync>(&self, exec: F) 
    where
        Self: 'static
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
        Self: 'static
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
    fn drain_dispatch_queues(&mut self) where Self: 'static {
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
}
