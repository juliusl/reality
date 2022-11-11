use specs::{
    shred::{Resource, ResourceId},
    Component, Join, World, WorldExt,
};
use std::{collections::HashMap, future::Future, ops::Deref};
use std::{
    fmt::Debug,
    io::{Read, Write},
};

use crate::{Block, Parser};

use super::{BlobDevice, ControlDevice, Encoder, Frame, WireObject};

pub mod async_ext;

/// Struct for protocol state
///
pub struct Protocol {
    /// Map of encoders for wire objects,
    ///
    encoders: HashMap<ResourceId, Encoder>,
    /// World used for storage,
    ///
    world: World,
    /// Enable to assert the entity generation that is created on decode,
    ///
    /// This can help ensure the integrity of transported frames.
    ///
    assert_generation: bool,
}

impl Protocol {
    /// Returns an empty protocol,
    ///
    pub fn empty() -> Self {
        Self {
            encoders: HashMap::default(),
            world: World::new(),
            assert_generation: false,
        }
    }

    /// Consumes a parser and returns new protocol,
    ///
    pub fn new(parser: Parser) -> Self {
        let mut protocol = Self {
            encoders: HashMap::default(),
            world: parser.commit(),
            assert_generation: false,
        };
        protocol.encode_components::<Block>();
        protocol
    }

    /// Returns self with assert_generation set to true
    ///
    pub fn enable_entity_generation_assert(mut self) -> Self {
        self.assert_generation = true;
        self
    }

    /// Returns true if decoding should assert entity generation,
    ///
    pub fn assert_entity_generation(&self) -> bool {
        self.assert_generation
    }

    /// Returns an iterator over encoders,
    ///
    pub fn iter_encoders(&self) -> impl Iterator<Item = (&ResourceId, &Encoder)> {
        self.encoders.iter()
    }

    /// Returns an encoder by id,
    ///
    pub fn encoder_mut_by_id(&mut self, id: ResourceId) -> Option<&mut Encoder> {
        self.encoders.get_mut(&id)
    }

    /// Takes an encoder from the protocol,
    ///
    pub fn take_encoder(&mut self, id: ResourceId) -> Option<Encoder> {
        self.encoders.remove(&id)
    }

    /// Encodes all components from the world,
    ///
    pub fn encode_components<T>(&mut self)
    where
        T: WireObject + Component,
        T::Storage: Default,
    {
        self.encoder::<T>(|world, encoder| {
            let components = world.read_component::<T>();
            for (e, c) in (&world.entities(), &components).join() {
                encoder.last_entity = Some(e);
                encoder.encode(c, world);
            }
        });
    }

    /// Encodes a resource from the world,
    ///
    pub fn encode_resource<T>(&mut self)
    where
        T: WireObject + Resource,
    {
        self.encoder::<T>(|world, encoder| {
            let resource = world.read_resource::<T>();

            encoder.encode(resource.deref(), world);
        });
    }

    /// Decodes and reads wire objects,
    ///
    pub async fn read<F, T>(&self, handle: impl Fn(&[Frame], T) -> F, complete: impl Fn(T) -> ())
    where
        T: WireObject,
        F: Future<Output = T>,
    {
        if let Some(encoder) = self.encoders.get(&T::resource_id()) {
            for (_, block_range) in encoder.frame_index() {
                for block_range in block_range {
                    let frames = &encoder.frames_slice()[block_range.clone()];

                    let obj = T::decode(&self, &encoder.interner, &encoder.blob_device, frames);

                    complete(handle(frames, obj).await)
                }
            }
        }
    }

    /// Decodes objects by resource id
    ///
    pub fn decode<T>(&self) -> Vec<T>
    where
        T: WireObject,
    {
        let mut c = vec![];

        if let Some(encoder) = self.encoders.get(&T::resource_id()) {
            for (name, block_range) in encoder.frame_index() {
                for block_range in block_range {
                    let start = block_range.start;
                    let end = block_range.end;

                    if end > encoder.frames_slice().len() {
                        panic!(
                            "Invalid range {name}, {end} > {},  {:#?}, {:#?}",
                            encoder.frames_slice().len(),
                            encoder.frame_index(),
                            encoder.frames_slice(),
                        );
                    }

                    let frames = &encoder.frames_slice()[start..end];

                    let obj = T::decode(
                        &self,
                        &encoder.interner,
                        &encoder.blob_device("decode").cursor(),
                        frames,
                    );
                    c.push(obj);
                }
            }
        }

        c
    }

    /// Returns a control device for resource,
    ///
    pub fn control_device<T>(&self) -> Option<ControlDevice>
    where
        T: WireObject,
    {
        if let Some(encoder) = self.encoders.get(&T::resource_id()).as_ref() {
            Some(ControlDevice::new(encoder.interner()))
        } else {
            None
        }
    }

    /// Returns a blob device for resource,
    ///
    pub fn blob_device<T>(&self) -> Option<BlobDevice>
    where
        T: WireObject,
    {
        if let Some(encoder) = self.encoders.get(&T::resource_id()).as_ref() {
            Some(encoder.blob_device(""))
        } else {
            None
        }
    }

    /// Returns an iterator over objects for transporting,
    ///
    pub fn iter_object_frames<T>(&self) -> impl Iterator<Item = &[Frame]>
    where
        T: WireObject,
    {
        if let Some(encoder) = self.encoders.get(&T::resource_id()) {
            encoder
                .frame_index
                .iter()
                .map(|(_, range)| range)
                .flatten()
                .cloned()
                .map(|r| &encoder.frames[r])
        } else {
            panic!("Protocol does not store resource {:?}", T::resource_id());
        }
    }

    /// Finds an encoder and calls encode,
    ///
    /// Returns the current number of frames encoded
    ///
    pub fn encoder<T>(&mut self, encode: impl FnOnce(&World, &mut Encoder)) -> usize
    where
        T: WireObject,
    {
        if let Some(encoder) = self.encoders.get_mut(&T::resource_id()) {
            encode(&self.world, encoder);
            encoder.frames.len()
        } else {
            let mut encoder = Encoder::new();
            encode(self.as_ref(), &mut encoder);
            let frame_count = encoder.frames.len();
            self.encoders.insert(T::resource_id(), encoder);
            frame_count
        }
    }

    /// Sends protocol data w/ streams
    ///
    pub fn send<T, W, F>(&mut self, control_stream: F, frame_stream: F, blob_stream: F)
    where
        T: WireObject,
        W: Write,
        F: FnOnce() -> W,
    {
        self.encoder::<T>(move |_, encoder| {
            let mut control_stream = control_stream();
            let control_stream = &mut control_stream;

            let control_device = ControlDevice::new(encoder.interner.clone());
            for f in control_device.data_frames() {
                assert_eq!(control_stream.write(f.bytes()).ok(), Some(64));
            }

            for f in control_device.read_frames() {
                assert_eq!(control_stream.write(f.bytes()).ok(), Some(64));
            }

            for f in control_device.index_frames() {
                assert_eq!(control_stream.write(f.bytes()).ok(), Some(64));
            }

            let mut frame_stream = frame_stream();
            let frame_stream = &mut frame_stream;

            for f in encoder.frames_slice() {
                assert_eq!(frame_stream.write(f.bytes()).ok(), Some(64));
            }

            let mut blob_stream = blob_stream();
            encoder.blob_device.set_position(0);

            let blob_len = encoder.blob_device.get_ref().len();
            assert_eq!(
                std::io::copy(&mut encoder.blob_device, &mut blob_stream).ok(),
                Some(blob_len as u64)
            );

            encoder.clear();
        });
    }

    /// Receive data for a protocol,
    ///
    /// This will replace the active interner, so it's best this is used with an empty protocol.
    ///
    pub fn receive<T, R, F>(&mut self, control_stream: F, frame_stream: F, blob_stream: F)
    where
        T: WireObject,
        R: Read,
        F: FnOnce() -> R,
    {
        self.encoder::<T>(move |_, encoder| {
            let mut control_device = ControlDevice::default();

            let mut control_stream = control_stream();

            let mut frame_buffer = [0; 64];
            while let Ok(()) = control_stream.read_exact(&mut frame_buffer) {
                let frame = Frame::from(frame_buffer);
                if frame.op() == 0x00 {
                    control_device.data.push(frame.clone());
                } else if frame.op() > 0x00 && frame.op() < 0x06 {
                    control_device.read.push(frame.clone());
                } else {
                    assert!(
                        frame.op() >= 0xC1 && frame.op() <= 0xC6,
                        "Index frames have a specific op code range"
                    );
                    control_device.index.push(frame.clone());
                }

                frame_buffer = [0; 64]
            }
            encoder.interner = control_device.into();

            let mut blob_stream = blob_stream();
            std::io::copy(&mut blob_stream, &mut encoder.blob_device).ok();

            let mut frame_stream = frame_stream();
            while let Ok(()) = frame_stream.read_exact(&mut frame_buffer) {
                let frame = Frame::from(frame_buffer);
                encoder.frames.push(frame);
                frame_buffer = [0; 64]
            }

            encoder.frame_index = T::build_index(&encoder.interner, &encoder.frames);
        });
    }

    /// Clears an encoder,
    ///
    pub fn clear<T>(&mut self)
    where
        T: WireObject,
    {
        if let Some(encoder) = self.encoders.get_mut(&T::resource_id()) {
            encoder.clear()
        }
    }
}

impl From<World> for Protocol {
    fn from(world: World) -> Self {
        Self {
            world,
            encoders: HashMap::default(),
            assert_generation: false,
        }
    }
}

impl AsRef<World> for Protocol {
    fn as_ref(&self) -> &World {
        &self.world
    }
}

impl AsMut<World> for Protocol {
    fn as_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl Debug for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Protocol")
            .field("encoders", &self.encoders)
            .field("assert_generation", &self.assert_generation)
            .finish()
    }
}

/// Tests decoding a block
///
#[test]
#[tracing_test::traced_test]
fn test_decode_block() {
    use atlier::system::Value;
    let protocol = Protocol::new(Parser::new().parse(
        r#"
    ``` call guest
    + address .text   localhost
    : protocol .symbol http
    : port     .int    8080
    ```
    "#,
    ));

    let blocks = protocol.decode::<Block>();
    let block = blocks.get(1).expect("should have a block");
    assert_eq!(block.name(), "call");
    assert_eq!(block.symbol(), "guest");
    assert_eq!(block.entity(), 1);
    let address = block.map_transient("address");
    assert_eq!(
        address.get("protocol"),
        Some(&Value::Symbol("http".to_string()))
    );
    assert_eq!(address.get("port"), Some(&Value::Int(8080)));
}
