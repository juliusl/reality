use specs::{
    shred::{Resource, ResourceId},
    Component, Join, World, WorldExt,
};
use std::fmt::Debug;
use std::{collections::HashMap, future::Future, ops::Deref};

use crate::{Block, Parser};

use super::{Encoder, Frame, WireObject};

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
        let resource_id = ResourceId::new::<T::Storage>();
        self.encoder(resource_id, |world, encoder| {
            let components = world.read_component::<T>();
            for (_, c) in (&world.entities(), &components).join() {
                // if e.id() > 0 {
                //     encoder.encode(c, world);
                // }
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
        let resource_id = ResourceId::new::<T>();

        self.encoder(resource_id, |world, encoder| {
            let resource = world.read_resource::<T>();

            encoder.encode(resource.deref(), world);
        });
    }

    /// Decodes and reads components,
    ///
    pub async fn read_components<F, T>(
        &self,
        handle: impl Fn(&[Frame], T) -> F,
        complete: impl Fn(T) -> (),
    ) where
        T: WireObject + Component,
        F: Future<Output = T>,
    {
        let resource_id = ResourceId::new::<T::Storage>();
        self.read(&resource_id, handle, complete).await;
    }

    /// Decodes and reads resources,
    ///
    pub async fn read_resources<F, T>(
        &self,
        handle: impl Fn(&[Frame], T) -> F,
        complete: impl Fn(T) -> (),
    ) where
        T: WireObject + Resource,
        F: Future<Output = T>,
    {
        let resource_id = ResourceId::new::<T>();
        self.read(&resource_id, handle, complete).await;
    }

    /// Decodes and reads wire objects,
    ///
    pub async fn read<F, T>(
        &self,
        resource_id: &ResourceId,
        handle: impl Fn(&[Frame], T) -> F,
        complete: impl Fn(T) -> (),
    ) where
        T: WireObject,
        F: Future<Output = T>,
    {
        if let Some(encoder) = self.encoders.get(&resource_id) {
            for (_, block_range) in encoder.frame_index() {
                for block_range in block_range {
                    let frames = &encoder.frames_slice()[block_range.clone()];

                    let obj = T::decode(&self, encoder, frames);

                    complete(handle(frames, obj).await)
                }
            }
        }
    }

    /// Decodes wire objects into a vector,
    ///
    pub fn decode_components<T>(&self) -> Vec<T>
    where
        T: WireObject + Component,
    {
        let resource_id = ResourceId::new::<T::Storage>();

        self.decode(&resource_id)
    }

    /// Decodes resources,
    ///
    pub fn decode_resources<T>(&self) -> Vec<T>
    where
        T: WireObject + Resource,
    {
        let resource_id = ResourceId::new::<T>();

        self.decode(&resource_id)
    }

    /// Decodes objects by resource id
    ///
    pub fn decode<T>(&self, resource_id: &ResourceId) -> Vec<T>
    where
        T: WireObject,
    {
        let mut c = vec![];

        if let Some(encoder) = self.encoders.get(&resource_id) {
            for (_, block_range) in encoder.frame_index() {
                for block_range in block_range {
                    let frames = &encoder.frames_slice()[block_range.clone()];

                    let obj = T::decode(&self, encoder, frames);
                    c.push(obj);
                }
            }
        }

        c
    }

    /// Finds an encoder and calls encode,
    ///
    fn encoder(&mut self, resource_id: ResourceId, encode: fn(&World, &mut Encoder)) {
        if let Some(encoder) = self.encoders.get_mut(&resource_id) {
            encode(&self.world, encoder);
        } else {
            let mut encoder = Encoder::new();
            encode(self.as_ref(), &mut encoder);
            self.encoders.insert(resource_id.clone(), encoder);
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

    let blocks = protocol.decode_components::<Block>();
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
