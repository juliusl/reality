mod call_async;
mod context;
mod plugin;
mod cache_ext;
mod kvp_ext;

pub mod prelude {
    pub use super::kvp_ext::*;
    pub use super::cache_ext::*;
    pub use super::call_async::*;
    pub use super::context::Context as ThunkContext;
    pub use super::plugin::NewFn;
    pub use super::plugin::Plugin;
    pub use crate::AsyncStorageTarget;
    use crate::Attribute;
    pub use crate::AttributeType;
    pub use crate::BlockObject;
    use crate::FieldRefController;
    pub use super::plugin::Pack;
    use crate::ResourceKey;
    pub use super::context::Remote;
    pub use super::context::Local;
    pub use crate::Shared;
    pub use crate::StorageTarget;
    pub use crate::ToFrame;
    pub use futures_util::Future;
    pub use futures_util::FutureExt;
    pub use std::marker::PhantomData;
    pub use std::ops::DerefMut;
    use crate::FieldPacket;
    use crate::SetField;

    /// Type alias for the fn passed by the THunk type,
    ///
    pub type ThunkFn = fn(ThunkContext) -> CallOutput;

    /// Wrapper struct for the enable frame fn,
    /// 
    #[derive(Clone)]
    pub struct EnableFrame(pub ThunkFn);

    /// Wrapper struct for the enable_virtual fn,
    /// 
    #[derive(Clone)]
    pub struct EnableVirtual(pub ThunkFn);

    /// Pointer-struct for normalizing plugin types,
    ///
    pub struct Thunk<P>(pub PhantomData<P>)
    where
        P: Plugin + Default + Send + Sync + 'static;

    impl<P, In> Plugin for Thunk<P> 
    where 
        P: Plugin<Virtual = In> + Send + Sync + 'static,
        In: FieldRefController + CallAsync + ToOwned<Owned = P> + NewFn<Inner = P> + Send + Sync
    {
        type Virtual = P::Virtual;
    }

    impl<P> Clone for Thunk<P>
    where
        P: Plugin + Default + Send + Sync + 'static,
    {
        fn clone(&self) -> Self {
            Self(self.0)
        }
    }

    impl<P> Default for Thunk<P>
    where
        P: Plugin + Default + Send + Sync + 'static,
    {
        fn default() -> Self {
            Self(PhantomData)
        }
    }

    impl<P> AttributeType<Shared> for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P>
    {
        fn symbol() -> &'static str {
            <P as AttributeType<Shared>>::symbol()
        }

        fn parse(parser: &mut crate::AttributeParser<Shared>, content: impl AsRef<str>) {
            <P as AttributeType<Shared>>::parse(parser, content);

            let key = parser.attributes.last().cloned().unwrap_or(ResourceKey::root());
            if let Some(storage) = parser.storage() {
                storage
                    .lazy_put_resource::<ThunkFn>(<P as Plugin>::call, key.transmute());
                storage
                    .lazy_put_resource::<EnableFrame>(EnableFrame(<P as Plugin>::enable_frame), key.transmute());
                storage
                    .lazy_put_resource::<EnableVirtual>(EnableVirtual(<P as Plugin>::enable_virtual), key.transmute());
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
        P: SetField<FieldPacket> + Plugin + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P>
    {
        /// Called when the block object is being loaded into it's namespace,
        ///
        async fn on_load(storage: AsyncStorageTarget<Shared>, rk: Option<ResourceKey<Attribute>>) {
            <P as BlockObject<Shared>>::on_load(storage, rk).await;
        }

        /// Called when the block object is being unloaded from it's namespace,
        ///
        async fn on_unload(storage: AsyncStorageTarget<Shared>, rk: Option<ResourceKey<Attribute>>) {
            <P as BlockObject<Shared>>::on_unload(storage, rk).await;
        }

        /// Called when the block object's parent attribute has completed processing,
        ///
        fn on_completed(storage: AsyncStorageTarget<Shared>) -> Option<AsyncStorageTarget<Shared>> {
            <P as BlockObject<Shared>>::on_completed(storage)
        }
    }

    impl<P> ToFrame for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P>
    {
        fn to_frame(&self, key: crate::ResourceKey<crate::Attribute>) -> crate::Frame {
            crate::Frame {
                fields: vec![],
                recv: self.receiver_packet(key),
            }
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
                CallOutput::Update(tc) => {

                    std::task::Poll::Ready(Ok(tc.take()))
                },
            }
        }
    }

    impl<P> SetField<FieldPacket> for Thunk<P> 
    where
        P: Plugin + Send + Sync + 'static,
    {
        fn set_field(&mut self, _: crate::FieldOwned<FieldPacket>) -> bool {
            false
        }
    }
}

pub use prelude::*;
