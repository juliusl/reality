use atlier::system::Value;

use super::{Encoder, Frame};
use std::{
    collections::VecDeque,
    io::{Read, Seek, Write},
    sync::Arc,
};

/// Struct for managing decoder state,
///
/// Borrows an encoder to read/decode frames and a
///
pub struct Decoder<'a, BlobImpl>
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// Frames being decoded,
    ///
    frames: VecDeque<Frame>,
    /// Encoder state being decoded,
    ///
    encoder: Arc<&'a Encoder<BlobImpl>>,
}

impl<'a, BlobImpl> Decoder<'a, BlobImpl>
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// Returns a new decoder w/ an encoder,
    /// 
    pub fn new(encoder: &'a Encoder<BlobImpl>) -> Self {
        Self {
            frames: VecDeque::from_iter(encoder.frames.iter().cloned()),
            encoder: Arc::new(encoder),
        }
    }

    /// Returns a new empty decoder,
    /// 
    pub fn empty(encoder: Arc<&'a Encoder<BlobImpl>>) -> Decoder<BlobImpl> {
        Decoder {
            frames: VecDeque::<Frame>::default(),
            encoder,
        }
    }

    /// Returns a new decoder if the front frame is an extension frame w/ the 
    /// matching namespace and symbol
    ///
    pub fn decode_extension(
        &mut self,
        namespace: impl AsRef<str>,
        symbol: impl AsRef<str>,
    ) -> Option<Decoder<BlobImpl>> {
        if let Some(front) = self.frames.front() {
            let _name = front
                .name(&self.encoder.interner)
                .expect("should have a name");
            let _symbol = front
                .symbol(&self.encoder.interner)
                .expect("should have a symbol");
            if front.is_extension() && _name == namespace.as_ref() && _symbol == symbol.as_ref() {
                let front = self.frames.pop_front().expect("should have a frame");
                let mut front_decoder = Self::empty(self.encoder.clone());

                while front_decoder.frames.len() < (front.frame_len() - 1) {
                    if let Some(frame) = self.frames.pop_front() {
                        front_decoder.frames.push_back(frame);
                    } else {
                        break;
                    }
                }

                Some(front_decoder)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns an iterator over frames in this decoder,
    /// 
    pub fn frames(&'a self) -> impl Iterator<Item = &'a Frame> {
        self.frames.iter()
    }

    /// Returns and decodes a value if the next frame is `add {name} .{attribute}`
    /// 
    pub fn decode_value(&mut self, name: impl AsRef<str>) -> Option<Value> {
        match self.frames.front() {
            Some(ref front) if front.is_add() => {
                let _name = front.name(&self.encoder.interner).expect("should have a name");
                if _name == name.as_ref() {
                    let front = self.frames.pop_front().expect("should have a frame");
                    front.read_value(&self.encoder.interner, &self.encoder.blob_device)
                } else {
                    None
                }
            },
            _ => None
        }
    }


    // Returns an decodes properties if the next frames are decoable into a block properties struct,
    //
    // pub fn decode_properties(&mut self, name: impl AsRef<str>) -> Option<BlockProperties> {
    //     match self.frames.front() {
    //         Some(ref front) if front.is_add() => {
    //             let _name = front.name(&self.encoder.interner).expect("should have a name");
    //             if _name == name.as_ref() {
    //                 let front = self.frames.pop_front().expect("should have a frame");
    //                 front.read_value(&self.encoder.interner, &self.encoder.blob_device)
    //             } else {
    //                 None
    //             }
    //         },
    //         _ => None
    //     }
    // }
}
