use reality::{StorageTarget, AsyncStorageTarget};

/// Trait to enable a type to participate in an update loop,
/// 
pub trait Update 
where
    Self: Sized + Send + Sync + 'static
{
    /// Return true to allow update to be called,
    /// 
    fn can_update(&self, storage: &impl StorageTarget) -> bool;

    /// Called if can_update returns true,
    /// 
    fn update<S: StorageTarget + Send + Sync + 'static>(&self, target: AsyncStorageTarget<S>) -> anyhow::Result<()>;
}