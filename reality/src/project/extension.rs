use std::pin::Pin;
use std::sync::Arc;

use anyhow::anyhow;
use futures_util::Future;

use crate::BlockObject;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;
use crate::ThunkContext;

/// Type-alias for a middleware fn,
///
type Middleware<C, T> = Arc<
    dyn Fn(ThunkContext, C, anyhow::Result<T>) -> anyhow::Result<(C, anyhow::Result<T>)>
        + Sync
        + Send
        + 'static,
>;

/// Type-alias for a middleware task,
///
type MiddlewareAsync<C, T> = Arc<
    dyn Fn(
            ThunkContext,
            C,
            anyhow::Result<T>,
        ) -> Pin<
            Box<
                dyn Future<Output = anyhow::Result<(C, anyhow::Result<T>)>> + Sync + Send + 'static,
            >,
        > + Send
        + Sync
        + 'static,
>;

/// Impl variable constraint for middleware,
///
/// **For non-async**
/// ```rs norun
/// impl Fn(Arc<C>, ThunkContext, anyhow::Result<T>) -> anyhow::Result<T>
///     + Sync
///     + Send
///     + 'static
/// ```
///
/// **Async version
/// ```rs norun
/// impl Fn(Arc<C>, ThunkContext, anyhow::Result<T>) -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Sync + Send + 'static>>
///     + Send
///     + Sync
///     + 'static
/// ```
macro_rules! impl_middleware_ty {
    () => {
        impl Fn(
            ThunkContext, C, anyhow::Result<T>) -> anyhow::Result<(C, anyhow::Result<T>)>
            + Sync
            + Send
            + 'static
    };
    (async) => {
        impl Fn(
            ThunkContext,
            C,
            anyhow::Result<T>,
        )
            -> Pin<Box<dyn Future<Output = anyhow::Result<(C, anyhow::Result<T>)>> + Sync + Send + 'static>>
        + Send
        + Sync
        + 'static
    }
}

/// Extension is an external facing callback that can be stored/retrieved programatically,
///
pub struct Transform<C, T>
where
    C: Send + Sync + 'static,
    T: BlockObject<Shared>,
{
    /// Middleware controller,
    ///
    controller: Option<C>,
    /// Resource-key for retrieving the underlying type,
    ///
    resource_key: Option<ResourceKey<anyhow::Result<(C, anyhow::Result<T>)>>>,
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

impl<C: Clone + Send + Sync + 'static, T: BlockObject<Shared>> Clone for Transform<C, T> {
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

impl<C, T> Transform<C, T>
where
    C: Send + Sync + 'static,
    T: BlockObject<Shared>,
{
    /// Runs the extension processing the pipeline,
    ///
    pub async fn run(&mut self, target: &mut ThunkContext, init: T) -> anyhow::Result<T> {
        if let Some(controller) = self.controller.take() {
            let key = self.resource_key;
            target
                .transient_mut()
                .await
                .put_resource(Ok((controller, anyhow::Ok::<T>(init))), key);

            let mut dispatcher = target
                .transient
                .dispatcher::<anyhow::Result<(C, anyhow::Result<T>)>>(self.resource_key)
                .await;
            dispatcher.enable().await;

            for before in self.before.iter() {
                let target = target.clone();
                let before = before.clone();

                dispatcher.queue_dispatch_owned(move |result| {
                    let (controller, value) = result?;
                    (before)(target, controller, value)
                });
            }
            for before_task in self.before_tasks.iter() {
                let target = target.clone();
                let before_task = before_task.clone();

                dispatcher.queue_dispatch_owned_task(move |result| {
                    Box::pin(async move { 
                        let (controller, value) = result?;
                        (before_task)(target, controller, value).await
                    })
                });
            }
            dispatcher.dispatch_all().await;

            if let Some(user) = self.user.clone() {
                let target = target.clone();
                dispatcher.queue_dispatch_owned(move |result| {
                    let (controller, value) = result?;
                    (user)(target, controller, value)
                });
            }
            if let Some(user_task) = self.user_task.clone() {
                let target = target.clone();
                dispatcher.queue_dispatch_owned_task(move |result| {
                    Box::pin(async move { 
                        let (controller, value) = result?;
                        (user_task)(target, controller, value).await
                    })
                });
            }
            dispatcher.dispatch_all().await;

            for after in self.after.iter() {
                let target = target.clone();
                let after = after.clone();

                dispatcher.queue_dispatch_owned(move |result| {
                    let (controller, value) = result?;
                    (after)(target, controller, value)
                });
            }
            for after_task in self.after_tasks.iter() {
                let target = target.clone();
                let after_task = after_task.clone();

                dispatcher.queue_dispatch_owned_task(move |result| {
                    Box::pin(async move { 
                        let (controller, value) = result?;
                        (after_task)(target, controller, value).await
                    })
                });
            }
            dispatcher.dispatch_all().await;

            let mut storage = target.transient_mut().await;
            if let Some(value) = storage.take_resource(self.resource_key) {
                let (controller, value) = (*value)?;
                self.controller = Some(controller);
                value
            } else {
                Err(anyhow!("Could not process pipeline"))
            }
        } else {
            Err(anyhow!("Could not process pipeline"))
        }
    }

    /// Returns a new extension,
    ///
    pub fn new(controller: C, resource_key: Option<ResourceKey<T>>) -> Self {
        Transform {
            controller: Some(controller),
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
    pub fn set_user(&mut self, middleware: impl_middleware_ty!()) {
        self.user = Some(Arc::new(Box::new(middleware)));
    }

    /// Sets the user_task middleware,
    ///
    #[inline]
    pub fn set_user_task(&mut self, middleware: impl_middleware_ty!(async)) {
        self.user_task = Some(Arc::new(middleware));
    }

    /// (Chainable) Sets the user middleware,
    ///
    #[inline]
    pub fn user(mut self, middleware: impl_middleware_ty!()) -> Self {
        self.set_user(middleware);
        self
    }

    /// (Chainable) Sets the user task middleware,
    ///
    #[inline]
    pub fn user_task(mut self, middleware: impl_middleware_ty!(async)) -> Self {
        self.set_user_task(middleware);
        self
    }

    /// Adds middleware to run before returning the inner type,
    ///
    #[inline]
    pub fn add_before(&mut self, middleware: impl_middleware_ty!()) {
        self.before.push(Arc::new(middleware));
    }

    /// Adds middleware to run after returning the inner type,
    ///
    #[inline]
    pub fn add_after(&mut self, middleware: impl_middleware_ty!()) {
        self.after.push(Arc::new(middleware));
    }

    /// (Chainable) Adds middleware to run before returning the inner type,
    ///
    #[inline]
    pub fn before(mut self, middleware: impl_middleware_ty!()) -> Self {
        self.add_before(middleware);
        self
    }

    /// (Chainable) Adds middleware to run after returning the inner type,
    ///
    #[inline]
    pub fn after(mut self, middleware: impl_middleware_ty!()) -> Self {
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
    pub fn add_before_task(&mut self, middleware: impl_middleware_ty!(async)) {
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
    pub fn add_after_task(&mut self, middleware: impl_middleware_ty!(async)) {
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
    pub fn before_task(mut self, middleware: impl_middleware_ty!(async)) -> Self {
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
    pub fn after_task(mut self, middleware: impl_middleware_ty!(async)) -> Self {
        self.add_after_task(middleware);
        self
    }
}

#[allow(unused_imports)]
mod tests {
    use std::cell::RefCell;

    use tokio::io::AsyncReadExt;
    use tracing::trace;

    use crate::{Transform, ResourceKey, Shared, StorageTarget};

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_extension() {
        let mut target = crate::ThunkContext::from(Shared::default().into_thread_safe());
        let time = tokio::time::Instant::now();

        let mut extension =
            Transform::<(), crate::project::Test>::new((), Some(ResourceKey::with_hash("test")));
        extension.add_before(move |_, c, t| {
            trace!("before called {:?}", time);
            Ok((c, t))
        });
        extension.set_user(move |_, c, t| {
            trace!("ok called {:?}", time);
            Ok((c, t))
        });

        let _ = extension
            .run(
                &mut target,
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
