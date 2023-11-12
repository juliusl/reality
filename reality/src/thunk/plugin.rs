use crate::prelude::*;

use super::prelude::CallAsync;
use super::prelude::CallOutput;
use super::prelude::ThunkContext;

/// Allows users to export logic as a simple fn,
///
pub trait Plugin: BlockObject<Shared> + CallAsync {
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
}
