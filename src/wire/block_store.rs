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

    /// The type of block builder,
    /// 
    type Builder: BlockBuilder;

    /// Includes interner w/ this block store,
    /// 
    fn include_interner(&mut self, interner: &Interner);

    /// Returns a join handle, whose result is the result of putting a block in the store,
    /// 
    fn build_block(&mut self, name: impl AsRef<str>) -> &mut Self::Builder;

    /// Returns a join handle, whose result is the completed store, 
    /// 
    fn finish(&mut self) -> FinishStore<Self::Store>;
}

/// Trait that abstracts building a single block in a block store,
/// 
pub trait BlockBuilder {
    /// Returns the name of this block,
    /// 
    fn name(&self) -> &String; 

    /// Puts a frame into the frame block data,
    /// 
    /// If the frame is an extent, then put_block should be called instead,
    /// 
    fn put_frame(&mut self, frame: &Frame);

    /// Returns a join handle, whose result is the result of putting a block in the store,
    /// 
    fn put_block(&mut self, frame: &Frame, blob: impl Into<Bytes>) -> PutBlock;

    /// Returns all frames as block data, 
    /// 
    fn frame_block_data(&self) -> Bytes; 

    /// Returns an ordered list of frames, which are stored as blocks,
    /// 
    fn ordered_block_list(&self) -> Vec<Frame>;
}
