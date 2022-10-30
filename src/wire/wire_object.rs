use specs::World;

use super::{Encoder, Frame, Protocol, encoder::FrameIndex, Interner};

/// Trait for encoding self into frames,
///
pub trait WireObject {
    /// Encodes self into frames,
    ///
    fn encode(&self, world: &World, encoder: &mut Encoder);

    /// Decodes frames into self,
    ///
    fn decode(protocol: &Protocol, encoder: &Encoder, frames: &[Frame]) -> Self;

    /// Build frame index,
    /// 
    fn build_index(interner: &Interner, frames: &[Frame]) -> FrameIndex; 
}
