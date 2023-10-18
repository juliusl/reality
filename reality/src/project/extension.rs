use std::pin::Pin;

use anyhow::anyhow;
use futures_util::Future;

use crate::AsyncStorageTarget;
use crate::BlockObject;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;

/// Type-alias for a middleware fn,
///
pub type Middleware<T> = fn(AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>;

/// Type-alias for a middleware task,
/// 
pub type MiddlewareAsync<T> = fn(AsyncStorageTarget<Shared>, anyhow::Result<T>) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>;

/// Extension is an external facing callback that can be stored/retrieved programatically,
///
#[derive(Clone)]
pub struct Extension<T>
where
    T: BlockObject<Shared>,
{
    /// Resource-key for retrieving the underlying type,
    ///
    resource_key: Option<ResourceKey<anyhow::Result<T>>>,
    /// List of middleware to run before user middleware,
    ///
    before: Vec<Middleware<T>>,
    /// List of middleware tasks to run before user middleware,
    /// 
    before_tasks: Vec<MiddlewareAsync<T>>,
    /// List of middleware to run after user middleware,
    ///
    after: Vec<Middleware<T>>,
    /// List of middleware tasks to run after user middleware,
    /// 
    after_tasks: Vec<MiddlewareAsync<T>>,
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
        initial.put_resource(anyhow::Ok::<T>(init), self.resource_key.clone());
        drop(initial);

        let mut dispatcher = target
            .dispatcher::<anyhow::Result<T>>(self.resource_key.clone())
            .await;
        dispatcher.enable().await;

        for before in self.before.iter() {
            let target = target.clone();
            let before = before.clone();

            dispatcher.queue_dispatch_owned(move |value| (before)(target, value));
        }
        for before_task in self.before_tasks.iter() {
            let target = target.clone();
            let before_task = before_task.clone();

            dispatcher.queue_dispatch_owned_task(move |value| (before_task)(target, value));
        }
        dispatcher.dispatch_all().await;

        {
            let target = target.clone();
            dispatcher.queue_dispatch_owned(move |value| user(target, value));
        }
        dispatcher.dispatch_all().await;

        for after in self.after.iter() {
            let target = target.clone();
            let after = after.clone();

            dispatcher.queue_dispatch_owned(move |value| (after)(target, value));
        }
        for after_task in self.after_tasks.iter() {
            let target = target.clone();
            let after_task = after_task.clone();

            dispatcher.queue_dispatch_owned_task(move |value| (after_task)(target, value));
        }
        dispatcher.dispatch_all().await;

        let mut storage = target.storage.write().await;
        if let Some(value) = storage.take_resource(self.resource_key.clone()) {
            *value
        } else {
            Err(anyhow!("Could not process pipeline"))
        }
    }

    /// Returns a new extension,
    ///
    pub fn new(resource_key: Option<ResourceKey<T>>) -> Extension<T> {
        Extension {
            resource_key: resource_key.map(|r| r.transmute()),
            before: vec![],
            after: vec![],
            before_tasks: vec![],
            after_tasks: vec![],
        }
    }

    /// Adds middleware to run before returning the inner type,
    ///
    #[inline]
    pub fn add_before(&mut self, middleware: Middleware<T>) {
        self.before.push(middleware.into());
    }

    /// Adds middleware to run after returning the inner type,
    ///
    #[inline]
    pub fn add_after(&mut self, middleware: Middleware<T>) {
        self.after.push(middleware.into());
    }

    /// (Chainable) Adds middleware to run before returning the inner type,
    ///
    #[inline]
    pub fn before(mut self, middleware: Middleware<T>) -> Self {
        self.before.push(middleware.into());
        self
    }

    /// (Chainable) Adds middleware to run after returning the inner type,
    ///
    #[inline]
    pub fn after(mut self, middleware: Middleware<T>) -> Self {
        self.after.push(middleware);
        self
    }

    /// Adds middleware to run before returning the inner type,
    ///
    /// **Usage Example** 
    /// ```rs norun
    /// extension.add_before_task(|storage, s| Box::pin(async { 
    ///     s
    /// }));
    /// ```
    #[inline]
    pub fn add_before_task(&mut self, middleware: MiddlewareAsync<T>) {
        self.before_tasks.push(middleware.into());
    }

    /// Adds middleware to run after returning the inner type,
    /// 
    /// **Usage Example** 
    /// ```rs norun
    /// extension.add_after_task(|storage, s| Box::pin(async { 
    ///     s
    /// }));
    /// ```
    #[inline]
    pub fn add_after_task(&mut self, middleware: MiddlewareAsync<T>) {
        self.after_tasks.push(middleware.into());
    }

    /// (Chainable) Adds middleware to run before returning the inner type,
    ///
    /// **Usage Example** 
    /// ```rs norun
    /// extension.before_task(|storage, s| Box::pin(async { 
    ///     s
    /// }));
    /// ```
    #[inline]
    pub fn before_task(mut self, middleware: MiddlewareAsync<T>) -> Self {
        self.before_tasks.push(middleware.into());
        self
    }

    /// (Chainable) Adds middleware to run after returning the inner type,
    ///
    /// **Usage Example** 
    /// ```rs norun
    /// extension.after_task(|storage, s| Box::pin(async { 
    ///     s
    /// }));
    /// ```
    #[inline]
    pub fn after_task(mut self, middleware: MiddlewareAsync<T>) -> Self {
        self.after_tasks.push(middleware);
        self
    }
}

#[allow(unused_imports)]
mod tests {
    use tokio::io::AsyncReadExt;
    use tracing::trace;

    use crate::{Extension, Shared, StorageTarget, ResourceKey};

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_extension() {
        let target = Shared::default().into_thread_safe();

        let mut extension = Extension::<crate::project::Test>::new(Some(ResourceKey::with_hash("test")));
        extension.add_before(|_, t| {
            trace!("before called");
            // let mut input = String::new();
            // std::io::stdin().read_line(&mut input)?;
            // println!("{}", input.trim());
            t
        });

        // extension.add_before_task(|_, s| Box::pin(async {
        //     let input = tokio::fs::read_to_string("Cargo.toml").await?;
        //     println!("{input}");
        //     s
        // }));

        let _ = extension
            .run(
                target,
                crate::project::Test {
                    name: "hello-world".to_string(),
                    file: "test".into(),
                },
                |_, s| {
                    trace!("ok called");
                    s
                },
            )
            .await;

            assert!(logs_contain("before called"));
            assert!(logs_contain("ok called"));
        ()
    }
}
