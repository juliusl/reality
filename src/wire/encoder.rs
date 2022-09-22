use super::{frame::Frame, BlobDevice, Interner};
use crate::Block;
use atlier::system::Value;
use std::{collections::BTreeMap, io::Cursor, ops::Range};

/// Encoder for encoding blocks to wire protocol for transport,
///
/// When encoding is completed, all blob data is collected into a single 
/// cursor
///
pub struct Encoder {
    /// String interner for storing identifiers
    ///
    interner: Interner,
    /// Cursor to a blob device for writing/reading extent data types
    ///
    blob_device: Cursor<Vec<u8>>,
    /// Frames that have been encoded
    ///
    frames: Vec<Frame>,
    /// Index of blocks added, uses the key format `{name} {symbol}`,
    /// the value is a range for the start, end frames for the block.
    ///
    block_index: BTreeMap<String, Range<usize>>,
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder {
    /// Returns a new encoder w/ an empty in-memory blob device
    ///
    pub fn new() -> Self {
        Self::new_with(Cursor::new(vec![]))
    }

    /// Returns a new encoder /w a blob_device
    ///
    pub fn new_with(blob_device: impl Into<Cursor<Vec<u8>>>) -> Self {
        Self {
            interner: Interner::default(),
            blob_device: blob_device.into(),
            frames: vec![],
            block_index: BTreeMap::new(),
        }
    }

    /// Returns a blob device using the current cursor state
    ///
    pub fn blob_device(&self, address: impl AsRef<str>) -> BlobDevice {
        BlobDevice::existing(address, &self.blob_device.clone())
    }

    /// Returns an interner using the current interned identifiers
    ///
    pub fn interner(&self) -> Interner {
        self.interner.clone()
    }

    /// Returns an iterator over frames
    ///
    pub fn iter_frames(&self) -> impl Iterator<Item = &Frame> {
        self.frames.iter()
    }

    /// Returns a slice of frames
    ///
    pub fn frames_slice(&self) -> &[Frame] {
        &self.frames
    }

    /// Returns the block index
    ///
    pub fn block_index(&self) -> &BTreeMap<String, Range<usize>> {
        &self.block_index
    }

    /// Encodes a block into frames
    ///
    pub fn encode_block(&mut self, block: &Block) {
        let mut idents = vec![block.name().to_string(), block.symbol().to_string()];

        // Scan attributes for identifiers
        for attr in block.iter_attributes() {
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
                    if let (
                        Value::Reference(key), 
                        Value::Complex(idents)
                    ) = (val.to_ref(), val) {
                        self.interner.insert_complex(key, idents);
                    }
                }
                _ => {}
            }
        }
        self.encode_intern_data(idents);

        let start = if self.frames.is_empty() {
            0
        } else {
            self.frames.len() - 1
        };

        self.frames
            .push(Frame::start_block(block.name(), block.symbol()));

        for attr in block.iter_attributes() {
            if attr.is_stable() {
                self.frames
                    .push(Frame::add(attr.name(), attr.value(), &mut self.blob_device));
            } else {
                let (name, symbol) = attr
                    .name()
                    .split_once("::")
                    .expect("expect transient name format");
                let (_, value) = attr.transient().expect("should be transient");

                self.frames
                    .push(Frame::define(name, symbol, value, &mut self.blob_device));
            }
        }
        self.frames.push(Frame::end_block());

        let end = self.frames.len();
        self.block_index
            .insert(format!("{} {}", block.name(), block.symbol()), start..end);
    }

    /// Encodes intern data,
    ///
    pub fn encode_intern_data(&mut self, identifiers: Vec<String>) {
        identifiers
            .iter()
            .map(|i| Value::Symbol(i.to_string()))
            .for_each(|s| {
                if let (Value::Reference(key), Value::Symbol(symbol)) = (s.to_ref(), s) {
                    self.interner.insert_string(key, symbol.to_string());
                } else {
                    unreachable!()
                }
            })
    }
}

#[test]
#[tracing_test::traced_test]
fn test_encoder() {
    use tracing::{event, Level};
    let content = r#"
    ``` call host 
    add address .text localhost 
    :: ipv6 .enable 
    :: path .text api/test 
    ``` guest 
    + address .text localhost
    :: ipv4 .enable
    :: path .text api/test2
    ```

    ``` test host 
    add address .text localhost
    ``` 

    ```
    + debug .enable 
    ``` guest
    + address .text testhost
    ```
    "#;

    let mut parser = crate::Parser::new().parse(content);
    parser.evaluate_stack();

    let mut encoder = Encoder::new();
    encoder.encode_block(parser.get_block("call", "guest"));
    encoder.encode_block(parser.get_block("call", "host"));
    encoder.encode_block(parser.get_block("test", "host"));
    encoder.encode_block(parser.get_block("", "guest"));
    encoder.encode_block(parser.root());

    // Test `call guest`
    let value = encoder.frames[3]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::TextBuffer("localhost".to_string()));

    let value = encoder.frames[2]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::Bool(true));

    let value = encoder.frames[1]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::TextBuffer("api/test2".to_string()));

    // Test `call host`
    let value = encoder.frames[8]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::TextBuffer("localhost".to_string()));

    let value = encoder.frames[7]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::Bool(true));

    let value = encoder.frames[6]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::TextBuffer("api/test".to_string()));

    for f in encoder.frames_slice() {
        event!(Level::TRACE, "{:#}", f);
    }

    let control_device = crate::wire::ControlDevice::new(encoder.interner.clone());
    // This is the size in memory 
    event!(Level::TRACE, "total memory size      : {} bytes", content.len());
    // When a string is serialized, this should be the size of that message w/ utf8 encoding
    event!(Level::TRACE, "total utf8 size        : {} bytes", content.chars().count() * 4);
    event!(Level::TRACE, "control_dev size       : {} frames {} total bytes", control_device.len(), control_device.size());
    event!(Level::TRACE, "encoded block size     : {} frames {} total bytes", &encoder.frames_slice().len(), &encoder.frames_slice().len() * 64);
    event!(Level::TRACE, "total blob device size : {} bytes", &encoder.blob_device("").size());
}
