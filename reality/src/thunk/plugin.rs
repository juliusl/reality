use crate::prelude::*;

use super::prelude::CallAsync;
use super::prelude::CallOutput;
use super::prelude::ThunkContext;

/// Allows users to export logic as a simple fn,
///
pub trait Plugin: BlockObject<Shared> + Clone + Default + CallAsync {
    /// Called when an event executes,
    ///
    /// Returning PluginOutput determines the behavior of the Event.
    ///
    fn call(context: ThunkContext) -> CallOutput {
        if context
            .filter
            .as_ref()
            .filter(|f| !Self::symbol().contains(*f))
            .is_some()
        {
            return CallOutput::Skip;
        }

        CallOutput::Spawn(context.spawn(|mut c| async {
            <Self as CallAsync>::call(&mut c).await?;
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
            unsafe { c.node_mut().await.put_resource(frame, c.attribute.map(|c| c.transmute())) }
            Ok(c)
        }))
    }
}
