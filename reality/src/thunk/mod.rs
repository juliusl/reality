mod cache_ext;
mod call_async;
mod context;
mod kvp_ext;
mod plugin;

pub mod prelude {
    pub use super::cache_ext::*;
    pub use super::call_async::*;
    pub use super::context::Context as ThunkContext;
    pub use super::context::Local;
    pub use super::context::LocalAnnotations;
    pub use super::context::Remote;
    pub use super::kvp_ext::*;
    pub use super::plugin::repr::PluginLevel;
    pub use super::plugin::repr::PluginRepr;
    pub use super::plugin::NewFn;
    pub use super::plugin::Pack;
    pub use super::plugin::Plugin;
    pub use crate::AsyncStorageTarget;
    pub use crate::AttributeType;
    use crate::AttributeTypeParser;
    pub use crate::BlockObject;
    pub use crate::Shared;
    pub use crate::StorageTarget;
    pub use crate::ToFrame;
    use async_trait::async_trait;
    pub use futures_util::Future;
    pub use futures_util::FutureExt;
    use runir::prelude::Linker;
    use runir::prelude::NodeLevel;
    use runir::prelude::RecvLevel;
    use runir::prelude::Repr;
    use runir::prelude::ResourceLevel;
    pub use std::marker::PhantomData;
    pub use std::ops::DerefMut;

    use crate::Attribute;
    use crate::AttributeParser;
    use crate::FieldPacket;
    use crate::FieldRefController;
    use crate::ParsedNode;
    use crate::ResourceKey;
    use crate::SetField;
    use runir::prelude::CrcInterner;
    use tracing::error;

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
        P: Plugin + runir::prelude::Recv + Default + Send + Sync + 'static;

    impl<P, In> Plugin for Thunk<P>
    where
        P: Plugin<Virtual = In> + runir::prelude::Recv + Send + Sync + 'static,
        In: FieldRefController + CallAsync + ToOwned<Owned = P> + NewFn<Inner = P> + Send + Sync,
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

    #[async_trait(?Send)]
    impl<P> runir::prelude::Recv for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P>,
    {
        fn symbol() -> &'static str {
            P::symbol()
        }

        /// Links a node level to a receiver and returns a new Repr,
        ///
        async fn link_recv(node: NodeLevel, fields: Vec<Repr>) -> anyhow::Result<Repr>
        where
            Self: Sized + Send + Sync + 'static,
        {
            let mut repr = Linker::new::<P>();
            let recv = RecvLevel::new::<P>(fields);
            repr.push_level(recv)?;
            repr.push_level(node.clone())?;
            repr.link().await
        }
    }

    impl<P> AttributeType<Shared> for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P>,
    {
        fn parse(parser: &mut crate::AttributeParser<Shared>, content: impl AsRef<str>) {
            <P as AttributeType<Shared>>::parse(parser, content);

            let key = parser
                .parsed_node
                .last()
                .cloned()
                .unwrap_or(ResourceKey::root());
            if let Some(storage) = parser.storage() {
                storage.lazy_put_resource(PluginLevel::new::<P>(), key.transmute())
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

    #[async_trait::async_trait(?Send)]
    impl<P> BlockObject for Thunk<P>
    where
        P: SetField<FieldPacket> + Plugin + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P>,
    {
        /// Returns the attribute-type parser for the block-object type,
        ///
        fn attribute_type() -> AttributeTypeParser<Shared> {
            AttributeTypeParser::new::<Self>(ResourceLevel::new::<P>())
        }

        /// Called when the block object is being loaded into it's namespace,
        ///
        async fn on_load(
            parser: AttributeParser<Shared>,
            storage: AsyncStorageTarget<Shared>,
            rk: Option<ResourceKey<Attribute>>,
        ) -> AttributeParser<Shared> {
            <P as BlockObject>::on_load(parser, storage, rk).await
        }

        /// Called when the block object is being unloaded from it's namespace,
        ///
        async fn on_unload(
            parser: AttributeParser<Shared>,
            storage: AsyncStorageTarget<Shared>,
            rk: Option<ResourceKey<Attribute>>,
        ) -> AttributeParser<Shared> {
            let parser = <P as BlockObject>::on_unload(parser, storage.clone(), rk).await;
            let _storage = storage.storage.read().await;

            if let Some(tl) = _storage.current_resource::<PluginLevel>(ResourceKey::root()) {
                drop(_storage);

                if let Some(mut parsed) = storage
                    .storage
                    .write()
                    .await
                    .resource_mut::<ParsedNode>(ResourceKey::root())
                {
                    if let Some(mut repr) = parsed.node.repr() {
                        if let Err(err) = repr.upgrade(CrcInterner::default(), tl).await {
                            error!("{err}");
                        }

                        parsed.node.set_repr(repr);
                    }
                    drop(parsed);
                }
            }

            parser
        }

        /// Called when the block object's parent attribute has completed processing,
        ///
        fn on_completed(storage: AsyncStorageTarget<Shared>) -> Option<AsyncStorageTarget<Shared>> {
            <P as BlockObject>::on_completed(storage)
        }
    }

    impl<P> ToFrame for Thunk<P>
    where
        P: Plugin + Send + Sync + 'static,
        P::Virtual: NewFn<Inner = P>,
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
                CallOutput::Update(tc) => std::task::Poll::Ready(Ok(tc.take())),
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
