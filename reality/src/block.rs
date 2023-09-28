use tokio::task::JoinHandle;

use crate::{AttributeTypeParser, StorageTarget};

/// Struct containing all attributes,
///
pub struct Block {}

pub trait BlockObject<Storage: StorageTarget> {
    /// Return a list of properties that can be defined w/ this object,
    /// 
    fn properties() -> Vec<AttributeTypeParser<Storage>>;

    /// Allows for a block object to be loaded as an extension,
    /// 
    #[cfg(feature = "async_dispatcher")]
    fn load_async() -> Result<JoinHandle<Self>, Self>
    where
        Self: Sized;
}

impl<Storage: StorageTarget> BlockObject<Storage> for () {
    fn properties() -> Vec<AttributeTypeParser<Storage>> {
        vec![]
    }

    fn load_async() -> Result<JoinHandle<Self>, Self>
    where
        Self: Sized,
    {
        Err(())
    }
}
