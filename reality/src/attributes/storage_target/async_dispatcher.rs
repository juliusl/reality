use futures_util::stream::FuturesOrdered;
use futures_util::Future;
use tokio::runtime::Handle;
use tracing::trace;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::prelude::*;

/// Wrapper for a thread_safe wrapper over a storage target type,
///
/// Provides a `dispatcher<T>` fn that enables and returns a dispatching queue for a stored resource.
///
pub struct AsyncStorageTarget<S: StorageTarget> {
    pub storage: Arc<RwLock<S>>,
    pub runtime: Option<Handle>,
}

impl<S: StorageTarget> Clone for AsyncStorageTarget<S> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            runtime: self.runtime.clone(),
        }
    }
}

impl<S: StorageTarget + Send + Sync + 'static> AsyncStorageTarget<S> {
    /// Creates an async storage target from its parts,
    /// 
    pub fn from_parts(storage: Arc<RwLock<S>>, runtime: tokio::runtime::Handle) -> Self {
        Self { storage, runtime: Some(runtime) }
    }

    /// Returns a dispatcher for a specific resource type,
    ///
    /// **Note**: If the dispatching queues were not already present this fn will add them.
    ///
    pub async fn dispatcher<T: Send + Sync + 'static>(
        &self,
        resource_key: Option<ResourceKey<T>>,
    ) -> Dispatcher<S, T> {
        let mut disp = Dispatcher {
            storage: self.clone(),
            resource_key,
            tasks: FuturesOrdered::new(),
            _u: PhantomData,
        };

        disp.enable().await;
        disp
    }

    /// Intializes the default value for T and enables dispatch queues,
    ///
    pub async fn intialize_dispatcher<T: Default + Send + Sync + 'static>(
        &self,
        resource_key: Option<ResourceKey<T>>,
    ) -> Dispatcher<S, T> {
        use std::ops::Deref;

        let dispatcher = self.dispatcher(resource_key).await;

        dispatcher
            .storage
            .storage
            .deref()
            .write()
            .await
            .put_resource(T::default(), resource_key);

        dispatcher
    }
}

/// Trait for a storage target to return a dispatcher for a stored resource,
///
pub struct Dispatcher<Storage: StorageTarget + Send + Sync + 'static, T: Send + Sync + 'static> {
    /// Thread-safe reference to the storage target,
    ///
    storage: AsyncStorageTarget<Storage>,
    /// Optional, resource_key of the resource as well as queues,
    ///
    resource_key: Option<ResourceKey<T>>,
    /// Handles lock acquisition,
    ///
    tasks: FuturesOrdered<JoinHandle<()>>,
    // Unused
    _u: PhantomData<T>,
}

impl<Storage: StorageTarget + Send + Sync + 'static, T: Send + Sync + 'static> Clone
    for Dispatcher<Storage, T>
{
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            resource_key: self.resource_key,
            tasks: FuturesOrdered::new(),
            _u: self._u,
        }
    }
}

/// Macro for queue-ing a function on a dispatch queue,
///
macro_rules! queue {
    ($rcv:ident, $queue:path, $exec:ident) => {
        use std::ops::Deref;

        if let Self {
            storage:
                AsyncStorageTarget {
                    storage,
                    runtime: Some(runtime),
                },
            resource_key,
            tasks,
            ..
        } = $rcv
        {
            let storage = storage.clone();
            let resource_id = resource_key.clone();

            tasks.push_back(runtime.spawn(async move {
                if let Some(queue) = storage
                    .deref()
                    .read()
                    .await
                    .resource::<$queue>(resource_id.map(|r| r.transmute()))
                {
                    if let Ok(mut queue) = queue.lock() {
                        queue.push_back(Box::new($exec));
                    }
                }
            }));
        }
    };
}

/// Macro for enabling dispatch queues,
///
macro_rules! enable_queue {
    ($rcv:ident, [$($queue_ty:path),*]) => {
        {
            use std::ops::Deref;

            let Self {
                storage: AsyncStorageTarget { storage, .. },
                resource_key,
                ..
            } = $rcv;

            $(
                let checking = storage.read().await;
                if checking
                    .resource::<$queue_ty>(resource_key.map(|r| r.transmute()))
                    .is_none()
                {
                    drop(checking);
                    let mut storage = storage.deref().write().await;
                    storage.put_resource(<$queue_ty as Default>::default(), resource_key.map(|r| r.transmute()));
                }
            )*
        }
    };
}

/// Macro for applying dispatches from a queue
///
macro_rules! dispatch {
    ($rcv:ident, $queue_ty:ident, $resource_ty:ident) => {
        use std::ops::Deref;

        let Self {
            storage: AsyncStorageTarget { storage, .. },
            resource_key,
            ..
        } = $rcv;

        let mut tocall = vec![];
        {
            let mut storage = storage.deref().write().await;
            let queue = storage
                .resource_mut::<$queue_ty<$resource_ty>>(resource_key.map(|r| r.transmute()));
            if let Some(queue) = queue {
                if let Ok(mut queue) = queue.lock() {
                    while let Some(func) = queue.pop_front() {
                        tocall.push(func);
                    }
                }
            }
        }
        {
            if let Some(mut resource) = storage
                .deref()
                .write()
                .await
                .resource_mut::<$resource_ty>(resource_key.map(|r| r.transmute()))
            {
                for call in tocall.drain(..) {
                    call(&mut resource);
                }
            }
        }
    };
}

/// Macro for applying dispatches from a queue
///
macro_rules! dispatch_async {
    ($rcv:ident, $queue_ty:ident, $resource_ty:ident) => {
        use std::ops::Deref;

        let Self {
            storage: AsyncStorageTarget { storage, .. },
            resource_key,
            ..
        } = $rcv;

        let mut tocall = vec![];
        {
            let mut storage = storage.deref().write().await;
            let queue = storage
                .resource_mut::<$queue_ty<$resource_ty>>(resource_key.map(|r| r.transmute()));
            if let Some(queue) = queue {
                if let Ok(mut queue) = queue.lock() {
                    while let Some(func) = queue.pop_front() {
                        tocall.push(func);
                    }
                }
            }
        }
        {
            if let Some(mut resource) = storage
                .deref()
                .write()
                .await
                .resource_mut::<$resource_ty>(resource_key.map(|r| r.transmute()))
            {
                for call in tocall.drain(..) {
                    call(&mut resource).await;
                }
            }
        }
    };
}

/// Macro for applying dispatches from a queue
///
macro_rules! dispatch_owned_async {
    ($rcv:ident, $queue_ty:ident, $resource_ty:ident) => {
        use std::ops::Deref;

        let Self {
            storage: AsyncStorageTarget { storage, .. },
            resource_key,
            ..
        } = $rcv;

        let mut tocall = vec![];
        {
            let mut storage = storage.deref().write().await;
            let queue = storage
                .resource_mut::<$queue_ty<$resource_ty>>(resource_key.map(|r| r.transmute()));
            if let Some(queue) = queue {
                if let Ok(mut queue) = queue.lock() {
                    while let Some(func) = queue.pop_front() {
                        tocall.push(func);
                    }
                }
            }
        }
        {
            let mut _outer = None;
            if let Some(resource) = storage
                .deref()
                .write()
                .await
                .take_resource::<$resource_ty>(resource_key.map(|r| r.transmute()))
            {
                let mut resource = *resource;
                for call in tocall.drain(..) {
                    resource = call(resource).await;
                }

                _outer = Some(resource);
            }
                
            if let Some(outer) = _outer {
                storage.deref().write().await.put_resource(outer, resource_key.map(|r| r.transmute()));
            }
        }
    };
}

macro_rules! dispatch_owned {
    ($rcv:ident, $queue_ty:ident, $resource_ty:ident) => {
        use std::ops::Deref;

        let Self {
            storage: AsyncStorageTarget { storage, .. },
            resource_key,
            ..
        } = $rcv;

        let mut tocall = vec![];
        {
            let mut storage = storage.deref().write().await;
            let queue = storage
                .resource_mut::<$queue_ty<$resource_ty>>(resource_key.map(|r| r.transmute()));
            if let Some(queue) = queue {
                if let Ok(mut queue) = queue.lock() {
                    while let Some(func) = queue.pop_front() {
                        tocall.push(func);
                    }
                }
            }
        }
        {
            let mut _outer = None;
            if let Some(resource) = storage
                .deref()
                .write()
                .await
                .take_resource::<$resource_ty>(resource_key.map(|r| r.transmute()))
            {
                _outer = Some(tocall.drain(..).fold(*resource, |resource, call| {
                    call(resource)
                }));
            }

            if let Some(outer) = _outer {
                storage.deref().write().await.put_resource(outer, resource_key.map(|r| r.transmute()));
            }
        }
    };
}

impl<'a, Storage: StorageTarget + Send + Sync + 'static, T: Send + Sync + 'static>
    Dispatcher<Storage, T>
{
    /// Dispatches all queued dispatches,
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
    pub async fn dispatch_all(&mut self)
    where
        Self: Send + Sync + 'static,
    {
        trace!("Handling dispatcher tasks");
        self.handle_tasks().await;
        trace!("Dispatching owned queue");
        self.dispatch_owned_queued().await;
        trace!("Dispatching owned task queue");
        self.dispatch_task_owned_queued().await;
        trace!("Dispatching mut queue");
        self.dispatch_mut_queued().await;
        trace!("Dispatching mut task queue");
        self.dispatch_mut_task_queued().await;
        trace!("Dispatching queue");
        self.dispatch_queued().await;
        trace!("Dispatching task queue");
        self.dispatch_task_queued().await;
    }

    /// Handle any pending tasks,
    ///
    /// Needs to be called before `dispatch_*_queued`.
    ///
    pub async fn handle_tasks(&mut self) {
        use futures_util::StreamExt;

        while (self.tasks.next().await).is_some() {}
    }

    /// Queues a dispatch fn w/ a reference to the target resource,
    ///
    pub fn queue_dispatch(&mut self, exec: impl FnOnce(&T) + 'static + Send + Sync)
    where
        Self: 'static,
    {
        queue!(self, DispatchQueue<T>, exec);
    }

    /// Queues a dispatch fn w/ a mutable reference to the target resource,
    ///
    pub fn queue_dispatch_mut(&mut self, exec: impl FnOnce(&mut T) + 'static + Send + Sync)
    where
        Self: 'static,
    {
        queue!(self, DispatchMutQueue<T>, exec);
    }

    /// Queues a dispatch fn w/ a mutable reference to the target resource,
    ///
    pub fn queue_dispatch_owned(&mut self, exec: impl FnOnce(T) -> T+ 'static + Send + Sync)
    where
        Self: 'static,
    {
        queue!(self, DispatchOwnedQueue<T>, exec);
    }

    /// Queues a dispatch fn w/ a mutable reference to the target resource,
    ///
    pub fn queue_dispatch_owned_task(
        &mut self,
        exec: impl FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) where
        Self: 'static,
    {
        queue!(self, DispatchOwnedTaskQueue<T>, exec);
    }

    /// Queues a dispatch task fn w/ a mutable reference to the target resource,
    /// 
    /// **Note**: There is no performance benefit over using this, since queues are synchronous when drained.
    /// 
    /// The only benefit is being able to use async code in the closure.
    /// 
    pub fn queue_dispatch_mut_task(
        &mut self,
        exec: impl FnOnce(&mut T) -> Pin<Box<dyn Future<Output = ()> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) where
        Self: 'static,
    {
        queue!(self, DispatchMutTaskQueue<T>, exec);
    }

    /// Queues a dispatch task fn w/ a reference to the storage target resource,
    ///
    /// **Note**: There is no performance benefit over using this, since queues are synchronous when drained.
    /// 
    /// The only benefit is being able to use async code in the closure.
    /// 
    pub fn queue_dispatch_task(
        &mut self,
        exec: impl FnOnce(&T) -> Pin<Box<dyn Future<Output = ()> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) where
        Self: 'static,
    {
        queue!(self, DispatchTaskQueue<T>, exec);
    }

    /// Enables dispatching for a resource type,
    ///
    pub async fn enable(&mut self) {
        enable_queue!(self, [DispatchQueue<T>,  DispatchMutQueue<T>, DispatchTaskQueue<T>, DispatchMutTaskQueue<T>, DispatchOwnedQueue<T>, DispatchOwnedTaskQueue<T>]);
    }

    /// Dispatches the mutable queue,
    ///
    pub async fn dispatch_mut_queued(&mut self)
    where
        Self: 'static,
    {
        dispatch!(self, DispatchMutQueue, T);
    }

    /// Dispatches the non-mutable queue,
    ///
    pub async fn dispatch_queued(&mut self)
    where
        Self: 'static,
    {
        dispatch!(self, DispatchQueue, T);
    }

    /// Dispatches the mutable task queue,
    ///
    pub async fn dispatch_mut_task_queued(&mut self)
    where
        Self: 'static,
    {
        dispatch_async!(self, DispatchMutTaskQueue, T);
    }

    /// Dispatches the non-mutable task queue,
    ///
    pub async fn dispatch_task_queued(&mut self)
    where
        Self: 'static,
    {
        dispatch_async!(self, DispatchTaskQueue, T);
    }

    /// Dispatches the mutable task queue,
    ///
    pub async fn dispatch_owned_queued(&mut self)
    where
        Self: 'static,
    {
        dispatch_owned!(self, DispatchOwnedQueue, T);
    }

    /// Dispatches the non-mutable task queue,
    ///
    pub async fn dispatch_task_owned_queued(&mut self)
    where
        Self: 'static,
    {
        dispatch_owned_async!(self, DispatchOwnedTaskQueue, T);
    }
}
