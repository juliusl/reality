cfg_specs! {
    pub mod specs;
}
cfg_async_dispatcher! {
    pub mod async_dispatcher;
}
pub mod simple;
pub mod complex;
pub mod target;
pub mod resource_key;

pub mod prelude {
    use std::pin::Pin;
    use futures_util::Future;

    /// Type-alias for a thread safe dispatch queue,
    ///
    pub(super) type DispatchQueue<S> =
        std::sync::Mutex<std::collections::VecDeque<Box<dyn FnOnce(&S) + 'static + Send + Sync>>>;

    /// Type-alias for a thread safe dispatch-mut queue,
    ///
    pub(super) type DispatchMutQueue<S> = std::sync::Mutex<
        std::collections::VecDeque<Box<dyn FnOnce(&mut S) + 'static + Send + Sync>>,
    >;

    /// Type-alias for a task fn,
    /// 
    pub(super) type TaskFn<S> = Box<dyn FnOnce(&S) -> Pin<Box<dyn Future<Output = ()> + Sync + Send + 'static>> + Send + Sync + 'static>;
    
    /// Type-alias for a mutable task fn,
    /// 
    pub(super) type MutTaskFn<S> = Box<dyn FnOnce(&mut S) -> Pin<Box<dyn Future<Output = ()> + Sync + Send + 'static>> + Send + Sync + 'static>;

    /// Type-alias for a thread safe dispatch task queue,
    ///
    pub(super) type DispatchTaskQueue<S> =  std::sync::Mutex<std::collections::VecDeque<TaskFn<S>>>;

    /// Type-alias for a thread safe dispatch mut task queue,
    /// 
    pub(super) type DispatchMutTaskQueue<S> =  std::sync::Mutex<std::collections::VecDeque<MutTaskFn<S>>>;

    pub use super::simple::Simple;
    pub use super::target::StorageTarget;
    pub use super::resource_key::ResourceKey;

    cfg_async_dispatcher! {
        pub use tokio::sync::RwLock;
        pub use super::complex::Complex;
        pub use super::async_dispatcher::AsyncStorageTarget;
        pub use super::async_dispatcher::Dispatcher;
    }

    cfg_specs! {
        pub use super::specs::*;
    }
}
