use anyhow::anyhow;

use crate::AsyncStorageTarget;
use crate::BlockObject;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;

/// Type-alias for a middle-ware fn,
///
pub type Middleware<T> = fn(AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>;

/// Extension is an external facing callback that can be stored/retrieved programatically,
///
#[derive(Clone)]
pub struct Extension<T>
where
    T: BlockObject<Shared>,
{
    /// Resource-key for retrieving the underlying type,
    ///
    resource_key: ResourceKey<anyhow::Result<T>>,
    /// List of middleware to run before returning inner type T,
    ///
    before: Vec<Middleware<T>>,
    /// List of middleware to run after returning inner type T,
    ///
    after: Vec<Middleware<T>>,
}

impl<T> Extension<T>
where
    T: BlockObject<Shared>,
{
    /// Runs the extension processing the pipeline,
    ///
    pub async fn run(
        &self,
        target: AsyncStorageTarget<Shared>,
        init: T,
        user: Middleware<T>,
    ) -> anyhow::Result<T> {
        let mut initial = target.storage.write().await;
        initial.put_resource(anyhow::Ok::<T>(init), Some(self.resource_key.clone()));
        drop(initial);

        let mut dispatcher = target
            .dispatcher::<anyhow::Result<T>>(Some(self.resource_key.clone()))
            .await;
        dispatcher.enable().await;

        for before in self.before.iter() {
            let target = target.clone();
            let before = before.clone();

            dispatcher.queue_dispatch_owned(move |value| (before)(target, value));
        }

        {
            let target = target.clone();
            dispatcher.queue_dispatch_owned(move |value| user(target, value));
        }

        for after in self.after.iter() {
            let target = target.clone();
            let after = after.clone();

            dispatcher.queue_dispatch_owned(move |value| (after)(target, value));
        }

        dispatcher.dispatch_owned_queued().await;

        let mut storage = target.storage.write().await;
        if let Some(value) = storage.take_resource(Some(self.resource_key.clone())) {
            *value
        } else {
            Err(anyhow!("Could not process pipeline"))
        }
    }

    /// Returns a new extension,
    ///
    pub fn new(resource_key: &ResourceKey<T>) -> Extension<T> {
        Extension {
            resource_key: resource_key.transmute(),
            before: vec![],
            after: vec![],
        }
    }
    
    /// Adds middleware to run before returning the inner type,
    ///
    #[inline]
    pub fn add_before(&mut self, middleware: impl Into<Middleware<T>>) {
        self.before.push(middleware.into());
    }

    /// Adds middleware to run after returning the inner type,
    ///
    #[inline]
    pub fn add_after(&mut self, middleware: impl Into<Middleware<T>>) {
        self.after.push(middleware.into());
    }

    /// (Chainable) Adds middleware to run before returning the inner type,
    ///
    #[inline]
    pub fn before(mut self, middleware: impl Into<Middleware<T>>) -> Self {
        self.before.push(middleware.into());
        self
    }

    /// (Chainable) Adds middleware to run after returning the inner type,
    ///
    #[inline]
    pub fn after(mut self, middleware: impl Into<Middleware<T>>) -> Self {
        self.after.push(middleware.into());
        self
    }
}
