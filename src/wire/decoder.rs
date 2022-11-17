use atlier::system::Value;

use crate::BlockProperties;

use super::{Encoder, Frame, Interner};
use std::{
    collections::{BTreeMap, VecDeque},
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
    pub fn decode_namespace(
        &mut self,
        namespace: impl AsRef<str>,
    ) -> BTreeMap<String, Decoder<'a, BlobImpl>> {
        let mut map = BTreeMap::default();
        while let Some(front) = self.frames.front() {
            let _name = front
                .name(&self.encoder.interner)
                .expect("should have a name");
            let _symbol = front
                .symbol(&self.encoder.interner)
                .expect("should have a symbol");

            if front.is_extension() && _name == namespace.as_ref() {
                if let Some(decoder) = self.decode_extension(namespace.as_ref(), &_symbol) {
                    map.insert(_symbol.to_string(), decoder);
                }
            }
        }

        map
    }

    /// Returns a new decoder if the front frame is an extension frame w/ the
    /// matching namespace and symbol
    ///
    pub fn decode_extension(
        &mut self,
        namespace: impl AsRef<str>,
        symbol: impl AsRef<str>,
    ) -> Option<Decoder<'a, BlobImpl>> {
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

    /// Returns the current interner,
    ///
    pub fn interner(&self) -> &Interner {
        &self.encoder.interner
    }

    /// Returns the current interner,
    ///
    pub fn blob_device(&self) -> &BlobImpl {
        &self.encoder.blob_device
    }

    /// Returns the next frame that would be decoded,
    ///
    pub fn peek(&'a self) -> Option<Frame> {
        self.frames.front().cloned()
    }

    /// Returns and decodes a value if the next frame is `add {name} .{attribute}`
    ///
    pub fn decode_value(&mut self, name: impl AsRef<str>) -> Option<Value> {
        match self.frames.front() {
            Some(ref front) if front.is_add() => {
                let _name = front
                    .name(&self.encoder.interner)
                    .expect("should have a name");
                if _name == name.as_ref() {
                    let front = self.frames.pop_front().expect("should have a frame");
                    front.read_value(&self.encoder.interner, &self.encoder.blob_device)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Consumes add frames until can no longer decode add frames,
    ///
    /// Panics if the current decoder does not have complete data.
    /// 
    pub fn decode_values(&mut self) -> Vec<(String, Value)> {
        let mut values = vec![];

        loop {
            match self.frames.front() {
                Some(ref front) if front.is_add() => {
                    let front = self.frames.pop_front().expect("should have a frame");
                    if let (Some(name), Some(value)) = (
                        front.name(&self.encoder.interner),
                        front.read_value(&self.encoder.interner, &self.encoder.blob_device),
                    ) {
                        values.push((name, value));
                    } else {
                        panic!("Expecting a name and value, current decoder has incomplete data");
                    }
                }
                _ => break,
            }
        }

        values
    }

    /// Will consume current frames into a block properties struct,
    ///
    /// The conditions for continuing to consume frames are:
    ///
    /// 1) The frame must be add or define, if an extension frame is encountered, this method will return
    /// 2) If the frame is a define, and the name does not match the block properties name, this method will return
    /// 3) If there are no more frames, this method will return
    ///
    pub fn decode_properties(&mut self, name: impl AsRef<str>) -> BlockProperties {
        let mut properties = BlockProperties::new(name.as_ref());

        loop {
            match self.frames.front() {
                Some(ref front) if front.is_add() => {
                    let _name = front
                        .name(&self.encoder.interner)
                        .expect("should have a name");

                    let front = self.frames.pop_front().expect("should have a frame");
                    if let Some(value) =
                        front.read_value(&self.encoder.interner, &self.encoder.blob_device)
                    {
                        properties.add(_name, value);
                    }
                }
                Some(ref front) if front.is_define() => {
                    let _name = front
                        .name(&self.encoder.interner)
                        .expect("should have a name");

                    if _name == name.as_ref() {
                        let front = self.frames.pop_front().expect("should have a frame");
                        let _name = front
                            .symbol(&self.encoder.interner)
                            .expect("should have a symbol");

                        if let Some(value) =
                            front.read_value(&self.encoder.interner, &self.encoder.blob_device)
                        {
                            properties.add(_name, value);
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        properties
    }
}
