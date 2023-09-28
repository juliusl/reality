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
    pub(super) use crate::attributes::Container;

    /// Type-alias for a thread safe dispatch queue,
    ///
    pub(super) type DispatchQueue<S> =
        std::sync::Mutex<std::collections::VecDeque<Box<dyn FnOnce(&S) + 'static + Send + Sync>>>;

    /// Type-alias for a thread safe dispatch-mut queue,
    ///
    pub(super) type DispatchMutQueue<S> = std::sync::Mutex<
        std::collections::VecDeque<Box<dyn FnOnce(&mut S) + 'static + Send + Sync>>,
    >;

    pub use super::simple::Simple;
    pub use super::target::StorageTarget;
    pub use super::target::StorageTargetCallbackProvider;
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
