mod call_async;
mod context;
mod plugin;

pub mod prelude {
    pub use super::call_async::*;
    pub use super::context::Context as ThunkContext;
    pub use super::plugin::Plugin;
    pub use crate::AsyncStorageTarget;
    pub use crate::AttributeType;
    pub use crate::BlockObject;
    pub use crate::SetupTransform;
    pub use crate::TransformPlugin;
    pub use crate::Shared;
    pub use crate::StorageTarget;
    pub use futures_util::Future;
    pub use futures_util::FutureExt;
    pub use std::marker::PhantomData;
    pub use std::ops::DerefMut;

    /// Type alias for the fn passed by the THunk type,
    ///
    pub type ThunkFn = fn(ThunkContext) -> CallOutput;

    /// Pointer-struct for normalizing plugin types,
    ///
    pub struct Thunk<P>(pub PhantomData<P>)
    where
        P: Plugin + Send + Sync + 'static;

    impl<P> Plugin for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
    {
    }

    impl<P> AttributeType<Shared> for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
    {
        fn symbol() -> &'static str {
            <P as AttributeType<Shared>>::symbol()
        }
        
        fn parse(parser: &mut crate::AttributeParser<Shared>, content: impl AsRef<str>) {
            <P as AttributeType<Shared>>::parse(parser, content);

            let key = parser.attributes.last();
            if let Some(storage) = parser.storage() {
                storage
                    .lazy_put_resource::<ThunkFn>(<P as Plugin>::call, key.map(|k| k.transmute()));
            }
        }
    }

    #[async_trait::async_trait]
    impl<P> CallAsync for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
    {
        ///
        async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
            <P as CallAsync>::call(context).await
        }
    }

    #[async_trait::async_trait]
    impl<P> BlockObject<Shared> for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
    {
        /// Called when the block object is being loaded into it's namespace,
        ///
        async fn on_load(storage: AsyncStorageTarget<Shared>) {
            <P as BlockObject<Shared>>::on_load(storage).await;
        }

        /// Called when the block object is being unloaded from it's namespace,
        ///
        async fn on_unload(storage: AsyncStorageTarget<Shared>) {
            <P as BlockObject<Shared>>::on_unload(storage).await;
        }

        /// Called when the block object's parent attribute has completed processing,
        ///
        fn on_completed(storage: AsyncStorageTarget<Shared>) -> Option<AsyncStorageTarget<Shared>> {
            <P as BlockObject<Shared>>::on_completed(storage)
        }
    }

    impl Future for CallOutput {
        type Output = anyhow::Result<Option<ThunkContext>>;

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            match self.deref_mut() {
                CallOutput::Spawn(task) => match task {
                    Some(handle) => match handle.poll_unpin(cx) {
                        std::task::Poll::Ready(output) => {
                            let context = output?.ok();
                            std::task::Poll::Ready(Ok(context))
                        }
                        std::task::Poll::Pending => {
                            cx.waker().wake_by_ref();
                            std::task::Poll::Pending
                        }
                    },
                    None => std::task::Poll::Ready(Ok(None)),
                },
                CallOutput::Abort(result) => match result {
                    Ok(_) => std::task::Poll::Ready(Ok(None)),
                    Err(err) => std::task::Poll::Ready(Err(anyhow::anyhow!("{err}"))),
                },
                CallOutput::Skip => std::task::Poll::Ready(Ok(None)),
            }
        }
    }
}

pub use prelude::*;
