use reality::{StorageTarget, ThunkContext};

use crate::engine::EngineHandle;

#[cfg(feature = "hyper-ext")]
pub mod hyper_ext;
#[cfg(feature = "poem-ext")]
pub mod poem_ext;
#[cfg(feature = "std-ext")]
pub mod std_ext;

/// General extensions for ThunkContext,
///
pub trait Ext {
    /// Returns an engine handle from storage,
    ///
    fn engine_handle(&self) -> Option<EngineHandle>;
}

impl Ext for ThunkContext {
    fn engine_handle(&self) -> Option<EngineHandle> {
        self.source
            .storage
            .try_read()
            .ok()
            .and_then(|s| s.resource::<EngineHandle>(None).map(|e| e.clone()))
    }
}
