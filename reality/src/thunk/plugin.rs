use crate::prelude::*;

use super::prelude::CallOutput;
use super::prelude::ThunkContext;
use super::prelude::CallAsync;

/// Allows users to export logic as a simple fn,
///
pub trait Plugin: BlockObject<Shared> + CallAsync {
    /// Called when an event executes,
    ///
    /// Returning PluginOutput determines the behavior of the Event.
    ///
    fn call(context: ThunkContext) -> CallOutput;
}
