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

    // /// Consumes a frame and returns a decoding task iff the next frame has the expected name,
    // ///
    // /// Returns None if there are no more frames to decode or the decoder has encountered a boundary.
    // ///
    // pub fn decode_next(&mut self, expect: impl AsRef<str>) -> Option<DecodingTask<'a, BlobImpl>> {
    //     if let Some(front) = self.frames.pop_front() {
    //         if let Some(name) = front.name(&self.encoder.interner) {
    //             if name != expect.as_ref() {
    //                 panic!("unexpected frame `{name}`, expected {}", expect.as_ref());
    //             }

    //             let mut children = None::<Vec<Frame>>;
    //             if front.frame_len() > 1 {
    //                 let mut _children = vec![];

    //                 while _children.len() < (front.frame_len() - 1) {
    //                     if let Some(frame) = self.frames.pop_front() {
    //                         _children.push(frame);
    //                     } else {
    //                         break;
    //                     }
    //                 }

    //                 children = Some(_children);
    //             }

    //             Some(DecodingTask::<'a> {
    //                 frame: front,
    //                 name: Some(name.to_string()),
    //                 symbol: None,
    //                 value: None,
    //                 interner: &self.encoder.interner,
    //                 blob_device: &self.encoder.blob_device,
    //                 children,
    //             })
    //         } else {
    //             // This means that there is a wire object encoding/decoding issue
    //             panic!("Could not decode name, incomplete interner")
    //         }
    //     } else {
    //         None
    //     }
    // }
}
