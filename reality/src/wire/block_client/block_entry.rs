use crate::wire::Frame;


/// Trait to access block entry metadata,
/// 
/// A block entry is a a collection of bytes w/ a parent Frame,
/// 
/// Examples are Extention frames via ExtensionToken which own a set of frames and
/// Add/Define frames w/ an extent value.
/// 
pub trait BlockEntry {
    /// Returns the frame representing the entry,
    /// 
    fn frame(&self) -> Frame; 
    
    /// Returns the size of this entry in bytes,
    /// 
    fn size(&self) -> usize; 
}

impl BlockEntry for () {
    fn frame(&self) -> Frame {
        Frame::end_block()
    }

    fn size(&self) -> usize {
        0
    }
}