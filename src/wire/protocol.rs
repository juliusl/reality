use specs::{
    shred::{Resource, ResourceId},
    Component, Join, World, WorldExt,
};
use std::fmt::Debug;
use std::{collections::HashMap, future::Future, ops::Deref};

use crate::{Block, Parser};

use super::{Encoder, Frame, WireObject, ControlDevice, BlobDevice};

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

    /// Encodes all components from the world,
    ///
    pub fn encode_components<T>(&mut self)
    where
        T: WireObject + Component,
        T::Storage: Default,
    {
        self.encoder::<T>(|world, encoder| {
            let components = world.read_component::<T>();
            for (_, c) in (&world.entities(), &components).join() {
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
    pub async fn read<F, T>(
        &self,
        handle: impl Fn(&[Frame], T) -> F,
        complete: impl Fn(T) -> (),
    ) where
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
            for (_, block_range) in encoder.frame_index() {
                for block_range in block_range {
                    let frames = &encoder.frames_slice()[block_range.clone()];

                    let obj = T::decode(&self, &encoder.interner, &encoder.blob_device("decode").cursor(), frames);
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
        T: WireObject
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
        T: WireObject 
    {
        if let Some(encoder) = self.encoders.get(&T::resource_id()).as_ref() {
            Some(encoder.blob_device(""))
        } else {
            None
        }
    }

    /// Returns an iterator over objects for transporting,
    ///
    pub fn iter_object_frames<T>(
        &self
    ) -> impl Iterator<Item = &[Frame]> 
    where
        T: WireObject
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
    pub fn encoder<T>(&mut self, encode: fn(&World, &mut Encoder)) 
    where
        T: WireObject
    {
        if let Some(encoder) = self.encoders.get_mut(&T::resource_id()) {
            encode(&self.world, encoder);
        } else {
            let mut encoder = Encoder::new();
            encode(self.as_ref(), &mut encoder);
            self.encoders.insert(T::resource_id(), encoder);
        } 
    }
}

impl AsRef<World> for Protocol {
    fn as_ref(&self) -> &World {
        &self.world
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
