use atlier::system::{Attribute, Value};
use specs::WorldExt;
use tracing::{event, Level};

use crate::{
    wire::{Encoder, Frame, FrameIndex, Interner, Protocol, WireObject},
    Block,
};

impl WireObject for Block {
    fn encode(&self, world: &specs::World, encoder: &mut Encoder) {
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

    fn decode(protocol: &Protocol, encoder: &Encoder, frames: &[Frame]) -> Self {
        let mut block = Block::default();
        let interner = encoder.interner();
        let blob = encoder.blob_device("decode_block");

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
                            .read_value(&interner, blob.cursor())
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
                        .read_value(&interner, blob.cursor())
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
}
