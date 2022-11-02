use std::io::{Cursor, Read, Write, Seek};

use atlier::system::{Attribute, Value};
use specs::{WorldExt, shred::ResourceId, Component};
use tracing::{event, Level};

use crate::{
    wire::{Encoder, Frame, FrameIndex, Interner, Protocol, WireObject},
    Block, BlockProperties, Keywords,
};

impl WireObject for BlockProperties {
    fn encode<BlobImpl>(&self, _: &specs::World, encoder: &mut Encoder<BlobImpl>)
    where
        BlobImpl: Read + Write + Seek + Clone + Default 
    {
        let mut frame = Frame::add(self.name(), &Value::Empty, &mut encoder.blob_device);

        if let Some(entity) = encoder.last_entity {
            frame = frame.with_parity(entity);
        }

        encoder.frames.push(frame);

        for (name, property) in self.iter_properties() {
            match property {
                crate::BlockProperty::Single(prop) => {
                    let mut frame = Frame::define(self.name(), name, prop, &mut encoder.blob_device);
                    if let Some(entity) = encoder.last_entity {
                        frame = frame.with_parity(entity);
                    }
                    encoder.frames.push(frame);
                },
                crate::BlockProperty::List(props) => {
                    for prop in props {
                        let mut frame = Frame::define(self.name(), name, prop, &mut encoder.blob_device);
                        if let Some(entity) = encoder.last_entity {
                            frame = frame.with_parity(entity);
                        }
                        encoder.frames.push(frame);
                    }
                },
                crate::BlockProperty::Required => {
                    let mut frame = Frame::define(self.name(), name, &Value::Symbol("{property:REQUIRED}".to_string()), &mut encoder.blob_device);
                    if let Some(entity) = encoder.last_entity {
                        frame = frame.with_parity(entity);
                    }
                    encoder.frames.push(frame);
                },
                crate::BlockProperty::Optional => {
                    let mut frame = Frame::define(self.name(), name, &Value::Symbol("{property:OPTIONAL}".to_string()), &mut encoder.blob_device);
                    if let Some(entity) = encoder.last_entity {
                        frame = frame.with_parity(entity);
                    }
                    encoder.frames.push(frame);
                },
                crate::BlockProperty::Empty => {
                    let mut frame = Frame::define(self.name(), name, &Value::Empty, &mut encoder.blob_device);
                    if let Some(entity) = encoder.last_entity {
                        frame = frame.with_parity(entity);
                    }
                    encoder.frames.push(frame);
                },
            }
        }
    }

    fn decode(protocol: &Protocol, interner: &Interner, blob_device: &Cursor<Vec<u8>>, frames: &[Frame]) -> Self {
        let root = frames.get(0).expect("should have a starting frame");

        let root_entity = root.get_entity(protocol.as_ref(), protocol.assert_entity_generation());

        assert!(root.op() == Keywords::Add as u8);

        let name = root.name(interner).expect("should have a name");

        let mut properties = BlockProperties::new(name);

        for frame in frames.iter().skip(1) {
            match frame.keyword() {
                Keywords::Define => {
                    let prop_entity = frame.get_entity(protocol.as_ref(), protocol.assert_entity_generation());
                    assert_eq!(root_entity, prop_entity);
                    properties.add(frame.symbol(interner).expect("should have a symbol"), frame.read_value(interner, blob_device).expect("should have a value"));
                }
                _ => {
                }
            }
        }

        properties
    }

    fn build_index(interner: &Interner, frames: &[Frame]) -> FrameIndex {
        let mut frame_index = FrameIndex::default();
        for (idx, frame) in frames.iter().enumerate() {
            if frame.keyword() == Keywords::Add {
                let key = format!("{}", frame.name(interner).expect("should have a name"));
                let range = if let Some(end) = frames[idx + 1..].iter().position(|f| f.keyword() == Keywords::Add) {
                    let range = idx..idx + end + 1;
                    assert!(range.start < range.end, "{:?}, {:?}", range, frames);
                    range
                } else {
                    let range = idx..frames.len();
                    assert!(range.start < range.end, "{:?}, {:?}", range, frames);
                    range
                };

                if let Some(props) = frame_index.get_mut(&key) {
                    props.push(range);
                } else {
                    frame_index.insert(key, vec![range]);
                }
            }
        }

        frame_index
    }

    fn resource_id() -> ResourceId {
        ResourceId::new::<<BlockProperties as Component>::Storage>()
    }
}

impl WireObject for Block {
    fn encode<BlobImpl>(&self, world: &specs::World, encoder: &mut Encoder<BlobImpl>) 
    where
        BlobImpl: Read + Write + Seek + Clone + Default
    {
        let mut idents = vec![self.name().to_string(), self.symbol().to_string()];

        // Scan attributes for identifiers
        for attr in self.iter_attributes() {
            let val = if attr.is_stable() {
                idents.push(attr.name.to_string());
                attr.value()
            } else {
                let (name, symbol) = attr
                    .name()
                    .split_once("::")
                    .expect("expect transient name format");

                idents.push(name.to_string());
                idents.push(symbol.to_string());

                &attr.transient().expect("transient should exist").1
            };

            match val {
                Value::Symbol(ident) => {
                    idents.push(ident.to_string());
                }
                Value::Complex(_) => {
                    if let (Value::Reference(key), Value::Complex(idents)) = (val.to_ref(), val) {
                        encoder.interner.insert_complex(key, idents);
                    }
                }
                _ => {}
            }
        }
        encoder.intern_identifiers(idents);

        let start = encoder.frames.len();

        let block_entity = world.entities().entity(self.entity());

        encoder
            .frames
            .push(Frame::start_block(self.name(), self.symbol()).with_parity(block_entity));

        for attr in self.iter_attributes() {
            let attr_entity = world.entities().entity(attr.id());
            if attr.is_stable() {
                encoder.frames.push(
                    Frame::add(attr.name(), attr.value(), &mut encoder.blob_device)
                        .with_parity(attr_entity),
                );
            } else {
                let (name, symbol) = attr
                    .name()
                    .split_once("::")
                    .expect("expect transient name format");
                let (_, value) = attr.transient().expect("should be transient");

                encoder.frames.push(
                    Frame::define(name, symbol, value, &mut encoder.blob_device)
                        .with_parity(attr_entity),
                );
            }
        }
        encoder
            .frames
            .push(Frame::end_block().with_parity(block_entity));

        let end = encoder.frames.len();

        let key = format!("{} {}", self.name(), self.symbol());

        if let Some(index) = encoder.frame_index.get_mut(&key) {
            index.push(start..end);
        } else {
            encoder.frame_index.insert(
                format!("{} {}", self.name(), self.symbol()),
                vec![start..end],
            );
        }
    }

    fn decode(protocol: &Protocol, interner: &Interner, blob: &Cursor<Vec<u8>>, frames: &[Frame]) -> Self {
        let mut block = Block::default();

        if let Some(start) = frames.get(0) {
            let name = start.name(&interner).unwrap_or_default();
            let symbol = start.symbol(&interner).unwrap_or_default();

            let entity = start.get_entity(protocol.as_ref(), protocol.assert_entity_generation());
            block = Block::new(entity, name, symbol)
        }

        for frame in frames.iter().skip(1) {
            let attr_entity =
                frame.get_entity(protocol.as_ref(), protocol.assert_entity_generation());

            if attr_entity.id() != block.entity() {
                event!(
                    Level::DEBUG,
                    "Found child entity in frame {} -> {}",
                    block.entity(),
                    attr_entity.id()
                );
            }

            match frame.keyword() {
                crate::parser::Keywords::Add => {
                    let attr = Attribute::new(
                        attr_entity.id(),
                        frame
                            .name(&interner)
                            .expect("frame must have a name to add attribute"),
                        frame
                            .read_value(&interner, blob)
                            .expect("frame must have a value to add attribute"),
                    );

                    block.add_attribute(&attr);
                }
                crate::parser::Keywords::Define => {
                    let name = frame
                        .name(&interner)
                        .expect("frame must have a name to define attribute");
                    let symbol = frame
                        .symbol(&interner)
                        .expect("frame must have a symbol to define attribute");
                    let value = frame
                        .read_value(&interner, blob)
                        .expect("frame must have value to define attribute");

                    let name = format!("{name}::{symbol}");
                    let mut attr = Attribute::new(attr_entity.id(), name, Value::Empty);
                    attr.edit_as(value);
                    block.add_attribute(&attr);
                }
                // Block delimitters are manually handled, so none should be in
                // the middle.
                crate::parser::Keywords::BlockDelimitter
                | crate::parser::Keywords::Comment
                | crate::parser::Keywords::Error => {}
            }
        }

        block
    }

    fn build_index(interner: &Interner, frames: &[Frame]) -> FrameIndex {
        let mut frame_index = FrameIndex::default();

        let mut entry = None::<(String, usize)>;

        for (idx, frame) in frames.iter().enumerate() {
            match frame.keyword() {
                crate::Keywords::BlockDelimitter => {
                    match (frame.name(interner), frame.symbol(interner)) {
                        (Some(name), Some(symbol)) => {
                            let key = format!("{} {}", name, symbol);
                            entry = Some((key, idx));
                        }
                        (None, Some(symbol)) => {
                            let key = format!(" {}", symbol);
                            entry = Some((key, idx));
                        }
                        _ => {
                            if let Some((key, start)) = entry.take() {
                                if let Some(index) = frame_index.get_mut(&key) {
                                    index.push(start..idx);
                                } else {
                                    frame_index.insert(key, vec![start..idx]);
                                }
                            }
                        }
                    }
                }
                _ => continue,
            }
        }

        frame_index
    }

    fn resource_id() -> specs::shred::ResourceId {
        ResourceId::new::<<Block as Component>::Storage>()
    }
}
