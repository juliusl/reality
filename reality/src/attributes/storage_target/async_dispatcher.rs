use std::marker::PhantomData;
use std::sync::Arc;

use tokio::pin;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::task::LocalSet;

use super::prelude::*;

/// Wrapper for a thread_safe wrapper over a storage target type,
///
/// Provides a `dispatcher<T>` fn that enables and returns a dispatching queue for a stored resource.
///
pub struct AsyncStorageTarget<S: StorageTarget>(pub Arc<RwLock<S>>);

impl<S: StorageTarget> Clone for AsyncStorageTarget<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: StorageTarget + 'static> AsyncStorageTarget<S> {
    /// Returns a dispatcher for a specific resource type,
    ///
    /// **Note**: If the dispatching queues were not already present this fn will add them.
    ///
    pub async fn dispatcher<T: Send + Sync + 'static>(
        &self,
        resource_key: Option<ResourceKey<T>>
    ) -> Dispatcher<S, T> {
        let mut disp = Dispatcher {
            storage: self.clone(),
            resource_key,
            local: LocalSet::new(),
            tasks: JoinSet::new(),
            _u: PhantomData,
        };

        disp.enable().await;
        disp
    }

    /// Intializes the default value for T and enables dispatch queues,
    ///
    pub async fn intialize_dispatcher<T: Default + Send + Sync + 'static>(
        &self,
        resource_key: Option<ResourceKey<T>>
    ) -> Dispatcher<S, T> {
        let dispatcher = self.dispatcher(resource_key.clone()).await;

        dispatcher
            .storage
            .0
            .deref()
            .write()
            .await
            .put_resource(T::default(), resource_key);

        dispatcher
    }
}

/// Trait for a storage target to return a dispatcher for a stored resource,
///
pub struct Dispatcher<Storage: StorageTarget, T: Send + Sync + 'static> {
    /// Thread-safe reference to the storage target,
    ///
    storage: AsyncStorageTarget<Storage>,
    /// Optional, resource_id of the resource as well as queues,
    ///
    resource_key: Option<ResourceKey<T>>,
    /// Allows tasks to be executed on the same thread,
    ///
    local: LocalSet,
    /// Handles lock acquisition,
    ///
    tasks: JoinSet<()>,
    // Unused
    _u: PhantomData<T>,
}

impl<Storage: StorageTarget + 'static, T: Send + Sync + 'static> Clone for Dispatcher<Storage, T> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            resource_key: self.resource_key.clone(),
            local: LocalSet::new(),
            tasks: JoinSet::new(),
            _u: self._u.clone(),
        }
    }
}

/// Macro for queue-ing a function on a dispatch queue,
///
macro_rules! queue {
    ($rcv:ident, $queue:path, $exec:ident) => {
        let Self {
            storage: AsyncStorageTarget(storage),
            resource_key,
            local,
            tasks,
            ..
        } = $rcv;

        let storage = storage.clone();
        let resource_id = resource_key.clone();

        let _local_set_guard = local.enter();

        tasks.spawn_local(async move {
            if let Some(queue) = storage.deref().read().await.resource::<$queue>(resource_id.map(|r| r.transmute())) {
                if let Ok(mut queue) = queue.lock() {
                    queue.push_back(Box::new($exec));
                }
            }
        });
    };
}

/// Macro for enabling dispatch queues,
/// 
macro_rules! enable_queue {
    ($rcv:ident, [$($queue_ty:path),*]) => {
        {
            let Self {
                storage: AsyncStorageTarget(storage),
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
        let Self {
            storage: AsyncStorageTarget(storage),
            resource_key,
            ..
        } = $rcv;

        let mut tocall = vec![];
        {
            let mut storage = storage.deref().write().await;
            let queue = storage.resource_mut::<$queue_ty<$resource_ty>>(resource_key.map(|r| r.transmute()));
            if let Some(queue) = queue {
                if let Ok(mut queue) = queue.lock() {
                    while let Some(func) = queue.pop_front() {
                        tocall.push(func);
                    }
                }
            }
        }
        {
            if let Some(mut resource) = storage.deref().write().await.resource_mut::<$resource_ty>(resource_key.map(|r| r.transmute())) {
                for call in tocall.drain(..) {
                    call(&mut resource);
                }
            }
        }
    };
}

impl<'a, Storage: StorageTarget + 'static, T: Send + Sync + 'static> Dispatcher<Storage, T> {
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
        Self: 'static,
    {
        self.handle_tasks().await;
        self.dispatch_mut_queued().await;
        self.dispatch_queued().await;
    }

    /// Handle any pending tasks,
    ///
    /// Needs to be called before `dispatch_*_queued`.
    ///
    pub async fn handle_tasks(&mut self) {
        // Required to pin because we do not want to consume the reference
        let local = &mut self.local;
        pin!(local);

        local.await;

        while let Some(_) = self.tasks.join_next().await {}
    }

        /// Queues a dispatch fn w/ a reference to the storage target,
    ///
    pub fn queue_dispatch(&mut self, exec: impl FnOnce(&T) + 'static + Send + Sync)
    where
        Self: 'static,
    {
        queue!(self, DispatchQueue<T>, exec);
    }

    /// Queues a dispatch fn w/ a mutable reference to the storage target,
    ///
    pub fn queue_dispatch_mut(&mut self, exec: impl FnOnce(&mut T) + 'static + Send + Sync)
    where
        Self: 'static,
    {
        queue!(self, DispatchMutQueue<T>, exec);
    }

    /// Enables dispatching for a resource type,
    ///
    pub async fn enable(&mut self) {
        enable_queue!(self, [DispatchQueue<T>,  DispatchMutQueue<T>]);
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
}
