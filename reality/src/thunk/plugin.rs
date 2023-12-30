use std::pin::Pin;
use tracing::debug;

use super::prelude::CallAsync;
use super::prelude::CallOutput;
use super::prelude::ThunkContext;
use crate::prelude::*;

/// Trait to provide a new fn for types that consume plugins,
///
/// Generated when the derive macro is used.
///
pub trait NewFn {
    /// The inner plugin type to create this type from,
    ///
    type Inner;

    /// Returns a new instance from plugin state,
    ///
    fn new(plugin: Self::Inner) -> Self;
}

/// Allows users to export logic as a simple fn,
///
pub trait Plugin: ToFrame + BlockObject + CallAsync + Clone + Default {
    /// Associated type of the virtual version of this plugin,
    ///
    /// **Note** If the derive macro is used, this type will be auto-generated w/ the plugin impl,
    ///
    type Virtual: FieldRefController + CallAsync + NewFn + Send + Sync + ToOwned;

    /// Called when an event executes,
    ///
    /// Returning PluginOutput determines the behavior of the Event.
    ///
    fn call(context: ThunkContext) -> CallOutput {
        CallOutput::Spawn(context.spawn(|mut c| async {
            <Self as CallAsync>::call(&mut c).await?;
            Ok(c)
        }))
    }

    /// Enables virtual mode for this plugin,
    ///
    fn enable_virtual(context: ThunkContext) -> CallOutput {
        CallOutput::Spawn(context.spawn(|mut c| async {
            <Self::Virtual as CallAsync>::call(&mut c).await?;
            Ok(c)
        }))
    }

    /// Converts initialized plugin into frame representation and stores
    /// the result to node storage.
    ///
    fn enable_frame(context: ThunkContext) -> CallOutput
    where
        Self::Virtual: NewFn<Inner = Self>,
    {
        CallOutput::Spawn(context.spawn(|c| async {
            debug!("Enabling frame");
            let init = c.initialized::<Self>().await;

            debug!("Converting to frame");
            let frame = init.to_frame(c.attribute);

            debug!("Creating frame listener");
            let listener = FrameListener::<Self>::new(init);

            debug!("Creating packet router");
            let packet_router = PacketRouter::<Self>::new(listener.routes());
            packet_router
                .dispatcher
                .set(c.dispatcher::<FrameUpdates>().await)
                .ok();

            let mut node = c.node.storage.write().await;
            debug!("Create packet routes for resource");
            node.maybe_put_resource(listener, c.attribute.transmute());

            debug!("Create packet routes for resource");
            node.maybe_put_resource(std::sync::Arc::new(packet_router), c.attribute.transmute());

            debug!("Putting frame for resource");
            node.maybe_put_resource(frame, c.attribute.transmute());

            drop(node);
            Ok(c)
        }))
    }

    /// Sync values from context,
    ///
    #[allow(unused_variables)]
    fn sync(&mut self, context: &ThunkContext) {}

    /// Listens for one packet,
    ///
    #[allow(unused_variables)]
    fn listen_one(
        router: std::sync::Arc<PacketRouter<Self>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}

pub trait Pack {
    /// Packs the receiver into storage,
    ///
    fn pack<S>(self, storage: &mut S)
    where
        S: StorageTarget;

    /// Unpacks self from Shared,
    ///
    /// The default value for a field will be used if not stored.
    ///
    fn unpack<S>(self, value: &mut S) -> Self
    where
        S: StorageTarget;
}

pub mod repr {
    use std::sync::Arc;

    use anyhow::anyhow;
    use runir::define_intern_table;
    use runir::prelude::*;
    use runir::push_tag;

    use crate::NewFn;
    use crate::Plugin;
    use crate::ThunkFn;
    use crate::ToFrame;

    define_intern_table!(CALL: ThunkFn);
    define_intern_table!(ENABLE_FRAME: ThunkFn);
    define_intern_table!(ENABLE_VIRTUAL: ThunkFn);

    /// Repr level containing plugin thunks,
    ///
    #[derive(Clone)]
    pub struct PluginLevel {
        /// Call thunk fn tag,
        ///
        call: Tag<ThunkFn, Arc<ThunkFn>>,
        /// Enable frame thunk fn tag,
        ///
        enable_frame: Tag<ThunkFn, Arc<ThunkFn>>,
        /// Enable virtual thunk fn tag,
        ///
        enable_virtual: Tag<ThunkFn, Arc<ThunkFn>>,
    }

    impl PluginLevel {
        /// Returns a new thunk level,
        ///
        pub fn new<P>() -> Self
        where
            P: Plugin,
            P::Virtual: NewFn<Inner = P>,
        {
            Self {
                call: Tag::new(&CALL, Arc::new(<P as Plugin>::call)),
                enable_frame: Tag::new(&ENABLE_FRAME, Arc::new(<P as Plugin>::enable_frame)),
                enable_virtual: Tag::new(&ENABLE_VIRTUAL, Arc::new(<P as Plugin>::enable_virtual)),
            }
        }

        /// Returns a new thunk level for P w/ data thunks from Inner,
        ///
        pub fn new_as<P, Inner>() -> Self
        where
            P: Plugin + Default + Clone + ToFrame + Send + Sync + 'static,
            P::Virtual: NewFn<Inner = Inner>,
            Inner: Plugin,
            Inner::Virtual: NewFn<Inner = Inner>,
        {
            Self {
                call: Tag::new(&CALL, Arc::new(<P as Plugin>::call)),
                enable_frame: Tag::new(&ENABLE_FRAME, Arc::new(<Inner as Plugin>::enable_frame)),
                enable_virtual: Tag::new(
                    &ENABLE_VIRTUAL,
                    Arc::new(<Inner as Plugin>::enable_virtual),
                ),
            }
        }
    }

    impl Level for PluginLevel {
        fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
            push_tag!(dyn interner, &self.call);
            push_tag!(dyn interner, &self.enable_frame);
            push_tag!(dyn interner, &self.enable_virtual);

            interner.set_level_flags(LevelFlags::LEVEL_4);

            interner.interner()
        }

        type Mount = ();

        fn mount(&self) -> Self::Mount {
            ()
        }
    }

    impl TryFrom<Repr> for PluginRepr {
        type Error = anyhow::Error;

        fn try_from(value: Repr) -> Result<Self, Self::Error> {
            if let Some(l) = value.get_levels().get(4) {
                Ok(PluginRepr(*l))
            } else {
                Err(anyhow!(
                    "Could not convert repr to plugin repr, missing level 4 representation"
                ))
            }
        }
    }

    /// Wrapper struct over intern handle providing access to plugin thunks,
    ///
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct PluginRepr(InternHandle);

    impl PluginRepr {
        /// Returns the call thunk,
        ///
        pub fn call(&self) -> Option<ThunkFn> {
            CALL.copy(&self.0)
        }

        /// Returns the enable_frame thunk,
        ///
        pub fn enable_frame(&self) -> Option<ThunkFn> {
            ENABLE_FRAME.copy(&self.0)
        }

        /// Returns the enable_virtual thunk,
        ///
        pub fn enable_virtual(&self) -> Option<ThunkFn> {
            ENABLE_VIRTUAL.copy(&self.0)
        }
    }
}
