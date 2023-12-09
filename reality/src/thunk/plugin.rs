use std::pin::Pin;

use tracing::debug;
use tracing::trace;

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
pub trait Plugin: ToFrame + BlockObject<Shared> + CallAsync + Clone + Default {
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
        Box::pin(async { () })
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
