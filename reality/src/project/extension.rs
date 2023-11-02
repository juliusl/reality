use std::pin::Pin;
use std::sync::Arc;

use anyhow::anyhow;
use futures_util::Future;

use crate::AsyncStorageTarget;
use crate::BlockObject;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;

/// Type-alias for a middleware fn,
///
type Middleware<C, T> = Arc<
    dyn Fn(Arc<C>, AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>
        + Sync
        + Send
        + 'static,
>;

/// Type-alias for a middleware task,
///
type MiddlewareAsync<C, T> = Arc<
    dyn Fn(
            Arc<C>,
            AsyncStorageTarget<Shared>,
            anyhow::Result<T>,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;

/// Extension is an external facing callback that can be stored/retrieved programatically,
///
pub struct Extension<C, T>
where
    C: Send + Sync + 'static,
    T: BlockObject<Shared>,
{
    /// Middleware controller,
    ///
    controller: Arc<C>,
    /// Resource-key for retrieving the underlying type,
    ///
    resource_key: Option<ResourceKey<anyhow::Result<T>>>,
    /// List of middleware to run before user middleware,
    ///
    before: Vec<Middleware<C, T>>,
    /// List of middleware tasks to run before user middleware,
    ///
    before_tasks: Vec<MiddlewareAsync<C, T>>,
    /// List of middleware to run after user middleware,
    ///
    after: Vec<Middleware<C, T>>,
    /// List of middleware tasks to run after user middleware,
    ///
    after_tasks: Vec<MiddlewareAsync<C, T>>,
    /// User middleware,
    ///
    user: Option<Middleware<C, T>>,
    /// User middleware task,
    ///
    user_task: Option<MiddlewareAsync<C, T>>,
}

impl<C: Send + Sync + 'static, T: BlockObject<Shared>> Clone for Extension<C, T> {
    fn clone(&self) -> Self {
        Self {
            controller: self.controller.clone(),
            resource_key: self.resource_key,
            before: self.before.clone(),
            before_tasks: self.before_tasks.clone(),
            after: self.after.clone(),
            after_tasks: self.after_tasks.clone(),
            user: self.user.clone(),
            user_task: self.user_task.clone(),
        }
    }
}

impl<C, T> Extension<C, T>
where
    C: Send + Sync + 'static,
    T: BlockObject<Shared>,
{
    /// Runs the extension processing the pipeline,
    ///
    pub async fn run(&self, target: AsyncStorageTarget<Shared>, init: T) -> anyhow::Result<T> {
        let mut initial = target.storage.write().await;
        initial.put_resource(anyhow::Ok::<T>(init), self.resource_key);
        drop(initial);

        let mut dispatcher = target
            .dispatcher::<anyhow::Result<T>>(self.resource_key)
            .await;
        dispatcher.enable().await;

        for before in self.before.iter() {
            let controller = self.controller.clone();
            let target = target.clone();
            let before = before.clone();

            dispatcher.queue_dispatch_owned(move |value| (before)(controller, target, value));
        }
        for before_task in self.before_tasks.iter() {
            let controller = self.controller.clone();
            let target = target.clone();
            let before_task = before_task.clone();

            dispatcher
                .queue_dispatch_owned_task(move |value| (before_task)(controller, target, value));
        }
        dispatcher.dispatch_all().await;

        if let Some(user) = self.user.clone() {
            let controller = self.controller.clone();
            let target = target.clone();
            dispatcher.queue_dispatch_owned(move |value| (user)(controller, target, value));
        }
        if let Some(user_task) = self.user_task.clone() {
            let controller = self.controller.clone();
            let target = target.clone();
            dispatcher.queue_dispatch_owned_task(move |value| user_task(controller, target, value));
        }
        dispatcher.dispatch_all().await;

        for after in self.after.iter() {
            let controller = self.controller.clone();
            let target = target.clone();
            let after = after.clone();

            dispatcher.queue_dispatch_owned(move |value| (after)(controller, target, value));
        }
        for after_task in self.after_tasks.iter() {
            let controller = self.controller.clone();
            let target = target.clone();
            let after_task = after_task.clone();

            dispatcher
                .queue_dispatch_owned_task(move |value| (after_task)(controller, target, value));
        }
        dispatcher.dispatch_all().await;

        let mut storage = target.storage.write().await;
        if let Some(value) = storage.take_resource(self.resource_key) {
            *value
        } else {
            Err(anyhow!("Could not process pipeline"))
        }
    }

    /// Returns a new extension,
    ///
    pub fn new(controller: C, resource_key: Option<ResourceKey<T>>) -> Self {
        Extension {
            controller: Arc::new(controller),
            resource_key: resource_key.map(|r| r.transmute()),
            before: vec![],
            after: vec![],
            before_tasks: vec![],
            after_tasks: vec![],
            user: None,
            user_task: None,
        }
    }

    /// Sets the user middleware,
    ///
    #[inline]
    pub fn set_user(
        &mut self,
        middleware: impl Fn(Arc<C>, AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>
            + Sync
            + Send
            + 'static,
    ) {
        self.user = Some(Arc::new(Box::new(middleware)));
    }

    /// Sets the user_task middleware,
    ///
    #[inline]
    pub fn set_user_task(
        &mut self,
        middleware: impl Fn(
                Arc<C>,
                AsyncStorageTarget<Shared>,
                anyhow::Result<T>,
            )
                -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) {
        self.user_task = Some(Arc::new(middleware));
    }

    /// (Chainable) Sets the user middleware,
    ///
    #[inline]
    pub fn user(
        mut self,
        middleware: impl Fn(Arc<C>, AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>
            + Sync
            + Send
            + 'static,
    ) -> Self {
        self.set_user(middleware);
        self
    }

    /// (Chainable) Sets the user task middleware,
    ///
    #[inline]
    pub fn user_task(
        mut self,
        middleware: impl Fn(
                Arc<C>,
                AsyncStorageTarget<Shared>,
                anyhow::Result<T>,
            )
                -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.set_user_task(middleware);
        self
    }

    /// Adds middleware to run before returning the inner type,
    ///
    #[inline]
    pub fn add_before(
        &mut self,
        middleware: impl Fn(Arc<C>, AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>
            + Sync
            + Send
            + 'static,
    ) {
        self.before.push(Arc::new(middleware));
    }

    /// Adds middleware to run after returning the inner type,
    ///
    #[inline]
    pub fn add_after(
        &mut self,
        middleware: impl Fn(Arc<C>, AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>
            + Sync
            + Send
            + 'static,
    ) {
        self.after.push(Arc::new(middleware));
    }

    /// (Chainable) Adds middleware to run before returning the inner type,
    ///
    #[inline]
    pub fn before(
        mut self,
        middleware: impl Fn(Arc<C>, AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>
            + Sync
            + Send
            + 'static,
    ) -> Self {
        self.add_before(middleware);
        self
    }

    /// (Chainable) Adds middleware to run after returning the inner type,
    ///
    #[inline]
    pub fn after(
        mut self,
        middleware: impl Fn(Arc<C>, AsyncStorageTarget<Shared>, anyhow::Result<T>) -> anyhow::Result<T>
            + Sync
            + Send
            + 'static,
    ) -> Self {
        self.add_after(middleware);
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
    pub fn add_before_task(
        &mut self,
        middleware: impl Fn(
                Arc<C>,
                AsyncStorageTarget<Shared>,
                anyhow::Result<T>,
            )
                -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) {
        self.before_tasks.push(Arc::new(middleware));
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
    pub fn add_after_task(
        &mut self,
        middleware: impl Fn(
                Arc<C>,
                AsyncStorageTarget<Shared>,
                anyhow::Result<T>,
            )
                -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) {
        self.after_tasks.push(Arc::new(middleware));
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
    pub fn before_task(
        mut self,
        middleware: impl Fn(
                Arc<C>,
                AsyncStorageTarget<Shared>,
                anyhow::Result<T>,
            )
                -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.add_before_task(middleware);
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
    pub fn after_task(
        mut self,
        middleware: impl Fn(
                Arc<C>,
                AsyncStorageTarget<Shared>,
                anyhow::Result<T>,
            )
                -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.add_after_task(middleware);
        self
    }
}

#[allow(unused_imports)]
mod tests {
    use tokio::io::AsyncReadExt;
    use tracing::trace;

    use crate::{Extension, ResourceKey, Shared, StorageTarget};

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_extension() {
        let target = Shared::default().into_thread_safe();
        let time = tokio::time::Instant::now();

        let mut extension =
            Extension::<(), crate::project::Test>::new((), Some(ResourceKey::with_hash("test")));
        extension.add_before(move |_, _, t| {
            trace!("before called {:?}", time);
            t
        });
        extension.set_user(move |_, _, t| {
            trace!("ok called {:?}", time);
            t
        });

        let _ = extension
            .run(
                target,
                crate::project::Test {
                    name: "hello-world".to_string(),
                    file: "test".into(),
                },
            )
            .await;

        assert!(logs_contain("before called"));
        assert!(logs_contain("ok called"));
        ()
    }
}
