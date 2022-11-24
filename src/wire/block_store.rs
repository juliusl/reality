use bytes::Bytes;

use super::{BlockClient, Frame, Interner};

/// Type alias for a task that will put a block in the store,
/// 
pub type PutBlock = tokio::task::JoinHandle<std::io::Result<()>>;
 
/// Type alias for a task that will finish building a store,
///
pub type FinishStore<Store> = tokio::task::JoinHandle<std::io::Result<Store>>;

/// Trait that abstracts building of a block store,
/// 
pub trait BlockStore {
    /// Block client this store returns,
    /// 
    type Client: BlockClient; 

    /// Store builder this store returns,
    /// 
    type Builder: BlockStoreBuilder<Store = Self>;

    /// Returns a client to the block store, if the store can be read,
    /// 
    fn client(&self) -> Option<Self::Client>;

    /// Returns a builder for this block store, if the store can be written to,
    /// 
    fn builder(&self) -> Option<Self::Builder>;

    /// Returns a reference to an interner,
    /// 
    fn interner(&self) -> &Interner;
}

/// Trait to abstract building a block store,
/// 
/// A block store stores a list of blobs indexed by a Frame, i.e. (Frame + Blob == Block)
/// 
pub trait BlockStoreBuilder {
    /// The type of store this builder is building,
    /// 
    type Store: BlockStore;

    /// Includes interner w/ this block store,
    /// 
    fn include_interner(&mut self, interner: &Interner);

    /// Returns a join handle, whose result is the result of putting a block in the store,
    /// 
    fn put_block(&mut self, frame: &Frame, blob: Option<impl Into<Bytes>>) -> PutBlock;

    /// Returns a join handle, whose result is the completed store, 
    /// 
    fn finish(&mut self) -> FinishStore<Self::Store>;
}