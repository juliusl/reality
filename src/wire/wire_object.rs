use std::io::{Cursor, Seek, Write, Read};

use specs::{World, shred::ResourceId};

use super::{Encoder, Frame, Protocol, encoder::FrameIndex, Interner};

/// Trait for encoding self into frames,
///
pub trait WireObject {
    /// Encodes self into frames,
    ///
    fn encode<BlobImpl>(&self, world: &World, encoder: &mut Encoder<BlobImpl>)
    where
        BlobImpl: Read + Write + Seek + Clone + Default;

    /// Decodes frames into self,
    ///
    fn decode(protocol: &Protocol, interner: &Interner, blob_device: &Cursor<Vec<u8>>, frames: &[Frame]) -> Self;

    /// Build frame index,
    /// 
    fn build_index(interner: &Interner, frames: &[Frame]) -> FrameIndex;

    /// Returns the resource id for this type,
    /// 
    fn resource_id() -> ResourceId;
}
