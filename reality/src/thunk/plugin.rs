use tracing::trace;

use crate::prelude::*;

use super::prelude::CallAsync;
use super::prelude::CallOutput;
use super::prelude::ThunkContext;

pub trait NewFn {
    type Inner;
    fn new(plugin: Self::Inner) -> Self;
}

/// Allows users to export logic as a simple fn,
///
pub trait Plugin: BlockObject<Shared> + CallAsync + Clone + Default {
    type Virtual: CallAsync + Send + Sync + ToOwned + NewFn;

    /// Called when an event executes,
    ///
    /// Returning PluginOutput determines the behavior of the Event.
    ///
    fn call(context: ThunkContext) -> CallOutput {
        // TODO
        // if context
        //     .filter
        //     .as_ref()
        //     .filter(|f| !Self::symbol().contains(*f))
        //     .is_some()
        // {
        //     return CallOutput::Skip;
        // }

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
    fn enable_frame(context: ThunkContext) -> CallOutput {
        CallOutput::Spawn(context.spawn(|c| async {
            let init = c.initialized::<Self>().await;
            let frame = init.to_frame(c.attribute);
            trace!("Putting frame for resource");
            unsafe {
                c.node_mut()
                    .await
                    .put_resource(frame, c.attribute.transmute())
            }
            Ok(c)
        }))
    }

    /// Sync values from context,
    ///
    #[allow(unused_variables)]
    fn sync(&mut self, context: &ThunkContext) {}
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
