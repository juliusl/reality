
use tokio::task::JoinHandle;

use crate::store::StoreIndex;

use super::BlockClient;

/// Type alias for a task that creates a new block transport from a source store index,
/// 
pub type TransportSource<T> = JoinHandle<std::io::Result<T>>;

/// Implementing this trait allows the transport of stored block data,
/// 
/// The main difference being that when bytes are read from the upstream source, they are read as-is, for example their compressed form,
/// This differs from the normal stream_range fn which returns that bytes that were written to the original blob device that encoded the stored data.
/// 
/// Where-as the block store trait focuses on building a block store from scratch, this focuses more on transporting block stores between storage implementations.
/// 
pub trait BlockTransport: Sized {
    /// Transport client type,
    /// 
    type TransportClient: BlockClient;

    /// Returns a new transport from a source index,
    ///
    fn transport<Client: BlockClient>(prefix: impl Into<String>, name: impl Into<String>, source: &StoreIndex<Client>) -> TransportSource<Self>;

    /// Returns a new block client,
    ///
    fn client(&self) -> Self::TransportClient;
}
