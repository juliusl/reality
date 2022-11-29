use std::io::{Seek, Write, Read};

use specs::{World, shred::ResourceId};

use super::{Encoder, Frame, Protocol, encoder::FrameIndex, Interner, Decoder};

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
    #[deprecated = "use decode_v2 instead"]
    fn decode<BlobImpl>(protocol: &Protocol<BlobImpl>, interner: &Interner, blob_device: &BlobImpl, frames: &[Frame]) -> Self
    where
        BlobImpl: Read + Write + Seek + Clone + Default;


    /// Uses a decoder to decode into self,
    /// 
    fn decode_v2<'a, BlobImpl>(_world: &World, _decoder: Decoder<'a, BlobImpl>) -> Self
    where
        Self: Sized,
        BlobImpl: Read + Write + Seek + Clone + Default 
    {
        todo!()
    }

    /// Build frame index,
    /// 
    fn build_index(interner: &Interner, frames: &[Frame]) -> FrameIndex;

    /// Returns the resource id for this type,
    /// 
    fn resource_id() -> ResourceId;
}
