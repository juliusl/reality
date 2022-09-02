use std::future::Future;

use atlier::system::{Attribute, Value};
use specs::{World, WorldExt};

use crate::{Block, Parser};

use super::{Encoder, Frame};

/// Struct for protocol state
///
pub struct Protocol {
    /// World that was used to create blocks
    ///
    world: World,
    /// Used to encode blocks into frames for transport
    ///
    encoder: Encoder,
}

impl Protocol {
    /// Returns new protocol state from a parser
    ///
    pub fn new(parser: Parser) -> Self {
        let mut encoder = Encoder::new();
        for block in parser.iter_blocks() {
            encoder.encode_block(block);
        }

        Self {
            encoder,
            world: parser.commit(),
        }
    }

    /// Replaces the current world w/ a new world 
    /// 
    pub fn reset_world(&mut self) {
        self.world = World::new();
        self.world.register::<Block>();

        self.world.entities().create();
    }

    /// Decodes blocks from encoder data, calls handle on each block
    /// decoded.
    ///
    /// Handle should return a future whose output is some result. That
    /// result will be passed to complete.
    ///
    pub async fn decode<F, T>(
        &self, 
        handle: impl Fn(Block) -> F, 
        complete: impl Fn(T) -> ()
    )
    where
        F: Future<Output = T>,
    {
        for (_, block_range) in self.encoder.block_index() {
            let frames = &self.encoder.frames_slice()[block_range.clone()];

            let block = self.decode_block(frames);

            complete(handle(block).await)
        }
    }

    /// Decode frames into a block
    ///
    fn decode_block(&self, block_frames: &[Frame]) -> Block {
        let mut block = Block::default();
        let interner = self.encoder.interner();
        let blob = self.encoder.blob_device("decode_block");

        if let Some(start) = block_frames.get(0) {
            let name = start
                .name(&interner)
                .expect("starting frame must have a name");
            let symbol = start
                .symbol(&interner)
                .expect("starting frame must have a symbol");
            let entity = self.world.entities().create();

            block = Block::new(entity, name, symbol)
        }

        for frame in block_frames.iter().skip(1) {
            match frame.keyword() {
                crate::parser::Keywords::Add => {
                    let attr = Attribute::new(
                        block.entity(),
                        frame
                            .name(&interner)
                            .expect("frame must have a name to add attribute"),
                        frame
                            .read_value(&interner.as_ref(), blob.cursor())
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
                        .read_value(interner.as_ref(), blob.cursor())
                        .expect("frame must have value to define attribute");
                
                    let name = format!("{name}::{symbol}");
                    let mut attr = Attribute::new(block.entity(), name, Value::Empty);
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
}

/// Tests decoding a block 
/// 
#[test]
#[tracing_test::traced_test]
fn test_decode_block() {
    let mut protocol = Protocol::new(Parser::new().parse(r#"
    ``` call guest
    add address .text   localhost
    :: protocol .symbol http
    :: port     .int    8080
    ```
    "#));

    let block = protocol.decode_block(protocol.encoder.frames_slice());
    assert_eq!(block.name(), "call");
    assert_eq!(block.symbol(), "guest");
    assert_eq!(block.entity(), 2); // because 0 is root, 1 is the one parsed, 2 is the one decoded

    let address = block.map_transient("address"); 
    assert_eq!(address.get("protocol"), Some(&Value::Symbol("http".to_string())));
    assert_eq!(address.get("port"), Some(&Value::Int(8080)));

    // Test using a new world
    protocol.reset_world();
    let block = protocol.decode_block(protocol.encoder.frames_slice());
    assert_eq!(block.name(), "call");
    assert_eq!(block.symbol(), "guest");
    assert_eq!(block.entity(), 1);

    let address = block.map_transient("address"); 
    assert_eq!(address.get("protocol"), Some(&Value::Symbol("http".to_string())));
    assert_eq!(address.get("port"), Some(&Value::Int(8080)));
}