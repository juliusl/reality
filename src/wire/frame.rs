use super::{BlobDevice, Data, Interner};
use crate::parser::{Attributes, Elements, Keywords};
use atlier::system::Value;
use bytemuck::cast;
use logos::Logos;
use rand::RngCore;
use specs::{Entity, World, WorldExt};
use std::{
    collections::HashMap,
    fmt::Display,
    io::{Cursor, Read, Seek, Write},
    ops::Range,
};
use tracing::{event, Level};

mod extension_token;
pub use extension_token::ExtensionToken;

/// A frame represents the data for a single operation,
///
/// At most a frame can be 64 bytes in length,
///
/// ## Frame layout
///
/// * An operation starts w/ a keyword represented by a single u8,
///
/// * Following the keyword are the arguments that describe the operation,
/// Ex. if the the frame is an `add` operation, than this will be an identifier and value.
///
/// * An operation frame will always have a value type represented by a single u8.
///
/// * Finally, the last portion of the frame will be used to retrieve data,
/// at most this will be: [u8; 16]
///
/// ## Example `add message .text hello world`
///
/// * [u8;  1] 0x0A                 - Keyword (add)
/// * [u8; 16] intern(message)      - Identifier (name)
/// * [u8;  1] 0x0A                 - Value type (.text)
/// * [u8; 16] extent(hello world)  - Value (hello world)
///
/// Layout: Byte, Chunk, Byte, Chunk
/// Frame size: 34 bytes
///
/// ## Example `define message encoding .symbol utf8`
///
/// * [u8;  1] 0x0D                 - Keyword (define)
/// * [u8; 16] intern(message)      - Identifier (name)
/// * [u8; 16] intern(encoding)     - Identifier (symbol)
/// * [u8;  1] 0x09                 - Value type (.symbol)
/// * [u8; 16] intern(utf8)         - Value (utf8)
///
/// Layout: Byte, Chunk, Chunk, Byte, Chunk
/// Frame size: 50 bytes
///
/// **Note** Above illustrates the difference between .symbol and .text
///
/// ## Example control frame - `0x00 helloworld`
///
/// Up to this point, the examples above have been block operations. The other
/// type of operation are control instructions. For example,
///
/// The segment from the title means to intern the string "helloworld",
///
/// The frames for this would be,
///
/// 1: 0x00 helloworld
/// 2: 0x01 0x0B 0x00
///
/// Each 0x00 instruction is a 63-byte chunk of control data,
///
/// Instructions 0x01-0x04 specify how to read from the control blob,
/// Each class must be processed completely, before the next class can be processed.
///
/// The above example is a single string, and a single class 1 instruction. Class 1,
/// means at most a 0xFF read can be made against control data. If 0x00 is encountered, this
/// ends that class, and the next class is processed.
///
/// As the above example implies, control frames cannot be compiled until there is
/// control data.
///
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    data: [u8; 64],
}

impl From<&[u8]> for Frame {
    fn from(slice: &[u8]) -> Self {
        let mut data = [0; 64];
        data.copy_from_slice(slice);

        Frame { data }
    }
}

impl Display for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "\n\t{:02x}", self.op())?;

            if let Some(attr) = self.attribute() {
                write!(
                    f,
                    " {}\n\t  {:016x} {:016x}\n\t  ",
                    attr,
                    self.name_key(),
                    self.symbol_key()
                )?;
                for b in self.value_bytes() {
                    write!(f, "{:x}", b)?;
                }
                write!(f, "\n\t  ")?;

                for b in self.parity_bytes() {
                    write!(f, "{:x}", b)?;
                }
                Ok(())
            } else {
                write!(f, "\n\t")?;
                for chunk in self.data().chunks(24) {
                    for b in chunk.iter() {
                        write!(f, "{:02x}", b)?;
                    }
                    write!(f, "\n\t")?;
                }
                Ok(())
            }
        } else {
            write!(f, "{}", base64::encode(self.data))
        }
    }
}

impl Frame {
    /// Returns a new instruction frame
    ///
    pub fn instruction(op: impl Into<u8>, argument: &[u8; 63]) -> Self {
        let mut data = [0; 64];
        data[0] = op.into();
        data[1..].copy_from_slice(argument);
        Self { data }
    }

    /// Returns a new frame for the block delimitter keyword in normal block mode
    ///
    pub fn start_block(name: impl AsRef<str>, symbol: impl AsRef<str>) -> Self {
        let name = Elements::lexer(name.as_ref())
            .next()
            .unwrap_or(Elements::Identifier("".to_string()));

        let symbol = Elements::lexer(symbol.as_ref())
            .next()
            .unwrap_or(Elements::Identifier("".to_string()));

        match (name, symbol) {
            (Elements::Identifier(name), Elements::Identifier(symbol)) => {
                let mut frame_builder = FrameBuilder::default();
                let mut written = 0;
                written += frame_builder
                    .write(Keywords::BlockDelimitter, None::<&mut Cursor<Vec<u8>>>)
                    .expect("can write");
                written += frame_builder
                    .write(
                        Elements::Identifier(name.to_string()),
                        None::<&mut Cursor<Vec<u8>>>,
                    )
                    .expect("can write");
                written += frame_builder
                    .write(
                        Elements::Identifier(symbol.to_string()),
                        None::<&mut Cursor<Vec<u8>>>,
                    )
                    .expect("can write");

                event!(
                    Level::TRACE,
                    "new frame for block start `{name}` `{symbol}`, size: {written}"
                );

                frame_builder.cursor.into()
            }
            // This is more strict than the parser implementation,
            _ => {
                panic!("Cannot create start block frame")
            }
        }
    }

    /// Returns a new frame for adding a stable attribute to a block
    ///
    pub fn add(
        name: impl AsRef<str>,
        value: &Value,
        blob: &mut (impl Read + Write + Seek + Clone),
    ) -> Self {
        if let Elements::Identifier(name) = Elements::lexer(name.as_ref())
            .next()
            .expect(&format!("should be valid identifier, {}", name.as_ref()))
        {
            let mut frame_builder = FrameBuilder::default();
            let mut written = 0;
            written += frame_builder
                .write(Keywords::Add, None::<&mut Cursor<Vec<u8>>>)
                .expect("can write");
            written += frame_builder
                .write(
                    Elements::Identifier(name.to_string()),
                    None::<&mut Cursor<Vec<u8>>>,
                )
                .expect("can write");

            let value_type: Attributes = value.into();
            written += frame_builder
                .write(value_type, None::<&mut Cursor<Vec<u8>>>)
                .expect("can write");
            written += frame_builder.write_value(value, blob).expect("can write");

            event!(
                Level::TRACE,
                "new frame for `add` `{name}`, size: {written}"
            );

            frame_builder.cursor.into()
        } else {
            panic!("invalid add syntax, expected identifier")
        }
    }

    /// Returns a new frame for adding a transient attribute to a block
    ///
    pub fn define(
        name: impl AsRef<str>,
        symbol: impl AsRef<str>,
        value: &Value,
        blob: &mut (impl Read + Write + Seek + Clone),
    ) -> Self {
        let name = Elements::lexer(name.as_ref())
            .next()
            .expect("should be valid identifier");

        let symbol = Elements::lexer(symbol.as_ref())
            .next()
            .expect("should be a valid identifier");

        match (name, symbol) {
            (Elements::Identifier(name), Elements::Identifier(symbol)) => {
                let mut frame_builder = FrameBuilder::default();
                let mut written = 0;
                written += frame_builder
                    .write(Keywords::Define, None::<&mut Cursor<Vec<u8>>>)
                    .expect("can write");
                written += frame_builder
                    .write(
                        Elements::Identifier(name.to_string()),
                        None::<&mut Cursor<Vec<u8>>>,
                    )
                    .expect("can write");
                written += frame_builder
                    .write(
                        Elements::Identifier(symbol.to_string()),
                        None::<&mut Cursor<Vec<u8>>>,
                    )
                    .expect("can write");

                let value_type: Attributes = value.into();
                written += frame_builder
                    .write(value_type, None::<&mut Cursor<Vec<u8>>>)
                    .expect("can write");
                written += frame_builder.write_value(value, blob).expect("can write");

                event!(
                    Level::TRACE,
                    "new frame for `define` `{name}` `{symbol}`, size: {written}"
                );

                frame_builder.cursor.into()
            }
            // This is more strict than the parser implementation,
            _ => {
                panic!("Cannot create start block frame")
            }
        }
    }

    /// Returns an extension frame,
    ///
    /// An extension frame does not parse into a block, but is a shortcut for defining
    /// custom wire objects that act like blocks.
    ///
    pub fn extension(namespace: impl AsRef<str>, symbol: impl AsRef<str>) -> Self {
        Self::start_extension(namespace, symbol).cursor.into()
    }

    /// Starts an extension framebuilder,
    ///
    /// An extension frame does not parse into a block, but is a shortcut for defining
    /// custom wire objects that act like blocks.
    ///
    pub fn start_extension(namespace: impl AsRef<str>, symbol: impl AsRef<str>) -> FrameBuilder {
        let namespace = Elements::lexer(namespace.as_ref())
            .next()
            .expect("should be valid identifier");

        let symbol = Elements::lexer(symbol.as_ref())
            .next()
            .expect("should be a valid identifier");

        match (namespace, symbol) {
            (Elements::Identifier(namespace), Elements::Identifier(symbol)) => {
                let mut frame_builder = FrameBuilder::default();
                let mut written = 0;
                written += frame_builder
                    .write(Keywords::Extension, None::<&mut Cursor<Vec<u8>>>)
                    .expect("can write");
                written += frame_builder
                    .write(
                        Elements::Identifier(namespace.to_string()),
                        None::<&mut Cursor<Vec<u8>>>,
                    )
                    .expect("can write");
                written += frame_builder
                    .write(
                        Elements::Identifier(symbol.to_string()),
                        None::<&mut Cursor<Vec<u8>>>,
                    )
                    .expect("can write");

                event!(
                    Level::TRACE,
                    "new frame for `extension` `{namespace}` `{symbol}`, size: {written}"
                );

                frame_builder
            }
            // This is more strict than the parser implementation,
            _ => {
                panic!("Cannot create extension frame")
            }
        }
    }

    /// Returns a new frame for closing the current block selection
    ///
    pub fn end_block() -> Self {
        let mut frame_builder = FrameBuilder::default();
        let _ = frame_builder
            .write(Keywords::BlockDelimitter, None::<&mut Cursor<Vec<u8>>>)
            .expect("can write");
        let _ = frame_builder
            .write(Data::Entropy, None::<&mut Cursor<Vec<u8>>>)
            .expect("can write");
        let _ = frame_builder
            .write(Data::Entropy, None::<&mut Cursor<Vec<u8>>>)
            .expect("can write");

        event!(Level::TRACE, "new frame for block end");

        frame_builder.cursor.into()
    }

    /// Gets the name value from the frame,
    ///
    /// **Caveat** The value must exist in the interner.
    ///
    pub fn name(&self, interner_data: &Interner) -> Option<String> {
        self.read_interned(1..17, interner_data.strings())
    }

    /// Gets the symbol value from the frame,
    ///
    /// **Caveat** The value must exist to the interner.
    ///
    pub fn symbol(&self, interner_data: &Interner) -> Option<String> {
        self.read_interned(17..33, interner_data.strings())
    }

    /// Length of frames (including this frame)
    /// 
    pub fn frame_len(&self) -> usize {
        match self.keyword() {
            Keywords::Extension => {
                // 1 byte - keyword
                // 16 bytes - namespace [1 ..17]
                // 16 bytes - symbol    [17..33]
                // 16 bytes - len       [33..49]

                let mut buffer = [0; 16];
                buffer.copy_from_slice(&self.bytes()[33..49]);
                let [len, _] = cast::<[u8; 16], [u64; 2]>(buffer);
                1 + len as usize
            },
            _ => 1
        }
    }

    /// Returns the name key,
    ///
    pub fn name_key(&self) -> u64 {
        let mut buffer = [0; 16];
        buffer.copy_from_slice(&self.data[1..17]);

        let data = cast::<[u8; 16], [u64; 2]>(buffer);
        data[0]
    }

    /// Returns the symbol key,
    ///
    pub fn symbol_key(&self) -> u64 {
        let mut buffer = [0; 16];
        buffer.copy_from_slice(&self.data[17..33]);

        let data = cast::<[u8; 16], [u64; 2]>(buffer);
        data[0]
    }

    /// Reads the current value from the frame,
    ///
    pub fn keyword(&self) -> Keywords {
        self.data[0].into()
    }

    /// Returns the length in bytes of the frame
    ///
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns the attribute value type if the frame contains
    /// a value,
    ///
    pub fn attribute(&self) -> Option<Attributes> {
        match self.keyword() {
            Keywords::Add => {
                event!(Level::TRACE, "decoding attribute value type for `add`");
                // byte, chunk,  byte <-- this value
                // 0,    |1..17  |17
                Some(self.data[17].into())
            }
            Keywords::Define => {
                event!(Level::TRACE, "decoding attribute value type for `define`");
                // byte,  chunk,  chunk,   byte <-- this value
                // 0      |1..17  |17..33  |33
                Some(self.data[33].into())
            }
            _ => None,
        }
    }

    /// Get the value bytes,
    ///
    pub fn value_bytes(&self) -> &[u8] {
        match self.keyword() {
            Keywords::Add => &self.data[18..34],
            Keywords::Define => &self.data[34..50],
            _ => &self.data[1..],
        }
    }

    /// Returns a slice from the parity range
    ///
    pub fn parity_bytes(&self) -> [u8; 8] {
        let mut parity_bytes = [0; 8];
        parity_bytes.copy_from_slice(&self.data[56..]);
        parity_bytes
    }

    /// Returns the entity from this frame
    ///
    pub fn get_entity(&self, world: &World, assert_generation: bool) -> Entity {
        let [id, gen] = cast::<[u8; 8], [u32; 2]>(self.parity_bytes());

        let entity = world.entities().entity(id);
        if assert_generation {
            let assert_generation = entity.gen().id() as u32;
            assert_eq!(assert_generation, gen);
        }

        entity
    }

    /// Read interned data from the frame,
    ///
    pub fn read_interned(
        &self,
        frame_range: Range<usize>,
        interner_data: &HashMap<u64, String>,
    ) -> Option<String> {
        let mut buffer = [0; 16];
        buffer.copy_from_slice(&self.data[frame_range]);

        let data = cast::<[u8; 16], [u64; 2]>(buffer);
        interner_data.get(&data[0]).map(|s| s.to_string())
    }

    /// Reads the current value from the frame,
    ///
    /// If the value is not inlined, then data must be read from
    /// either the interner data, or a cursor representing a blob device.
    ///
    pub fn read_value(&self, interner: &Interner, blob_device: &Cursor<Vec<u8>>) -> Option<Value> {
        let mut blob_device = blob_device.clone();
        let value_offset = match self.keyword() {
            Keywords::Add => 18,
            Keywords::Define => 34,
            _ => {
                panic!("frame does not have a value")
            }
        };

        match self.attribute() {
            Some(value_type) => {
                let data = &self.data[value_offset..];

                match value_type {
                    Attributes::Empty => Some(Value::Empty),
                    Attributes::Bool => {
                        if data[0] == 0x01 {
                            Some(Value::Bool(true))
                        } else {
                            Some(Value::Bool(false))
                        }
                    }
                    Attributes::Int => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [val, ..] = cast::<[u8; 16], [i32; 4]>(buffer);

                        Some(Value::Int(val))
                    }
                    Attributes::IntPair => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [a, b, ..] = cast::<[u8; 16], [i32; 4]>(buffer);

                        Some(Value::IntPair(a, b))
                    }
                    Attributes::IntRange => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [a, b, c, ..] = cast::<[u8; 16], [i32; 4]>(buffer);

                        Some(Value::IntRange(a, b, c))
                    }
                    Attributes::Float => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [a, ..] = cast::<[u8; 16], [f32; 4]>(buffer);

                        Some(Value::Float(a))
                    }
                    Attributes::FloatPair => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [a, b, ..] = cast::<[u8; 16], [f32; 4]>(buffer);

                        Some(Value::FloatPair(a, b))
                    }
                    Attributes::FloatRange => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [a, b, c, ..] = cast::<[u8; 16], [f32; 4]>(buffer);

                        Some(Value::FloatRange(a, b, c))
                    }
                    Attributes::Symbol => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [key, ..] = cast::<[u8; 16], [u64; 2]>(buffer);

                        interner
                            .strings()
                            .get(&key)
                            .map(|d| Value::Symbol(d.to_string()))
                    }
                    Attributes::Text => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [len, cursor] = cast::<[u8; 16], [u64; 2]>(buffer);

                        blob_device.set_position(cursor);

                        let mut buf = vec![0; len as usize];

                        match blob_device.read_exact(&mut buf) {
                            Ok(_) => match String::from_utf8(buf) {
                                Ok(value) => Some(Value::TextBuffer(value)),
                                Err(err) => {
                                    event!(Level::ERROR, "could not parse utf8, {err}");
                                    None
                                }
                            },
                            Err(err) => {
                                event!(Level::ERROR, "could not read from blob device, {err}");
                                None
                            }
                        }
                    }
                    Attributes::BinaryVector => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [len, cursor] = cast::<[u8; 16], [u64; 2]>(buffer);
                        blob_device.set_position(cursor);

                        let mut buf = vec![0; len as usize];
                        match blob_device.read_exact(&mut buf) {
                            Ok(_) => Some(Value::BinaryVector(buf.to_vec())),
                            Err(err) => {
                                event!(Level::ERROR, "could not read from blob device, {err}");
                                None
                            }
                        }
                    }
                    Attributes::Complex => {
                        let mut buffer = [0; 16];
                        buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                        let [key, ..] = cast::<[u8; 16], [u64; 2]>(buffer);

                        interner
                            .complexes()
                            .get(&key)
                            .map(|d| Value::Complex(d.clone()))
                    }
                    Attributes::Identifier | Attributes::Error | Attributes::Comment => {
                        panic!("frame does not have a value type")
                    }
                }
            }
            None => None,
        }
    }

    /// Returns value data from the frame,
    ///
    pub fn value(&self) -> Option<Data> {
        let value_offset = match self.keyword() {
            Keywords::Add => 18,
            Keywords::Define => 34,
            _ => {
                panic!("frame does not have a value")
            }
        };

        match self.attribute() {
            Some(attr) => match attr {
                Attributes::Empty => Some(Data::InlineEmpty),
                Attributes::Bool => Some(if self.data[value_offset..][0] == 0x01 {
                    Data::InlineTrue
                } else {
                    Data::InlineFalse
                }),
                Attributes::Int
                | Attributes::IntPair
                | Attributes::IntRange
                | Attributes::Float
                | Attributes::FloatPair
                | Attributes::FloatRange => {
                    let mut data = [0; 16];
                    data.copy_from_slice(&self.data[value_offset..value_offset + 16]);

                    Some(Data::Inline { data })
                }
                Attributes::Symbol | Attributes::Complex | Attributes::Identifier => {
                    let mut buffer = [0; 16];
                    buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                    let [key, ..] = cast::<[u8; 16], [u64; 2]>(buffer);

                    Some(Data::Interned { key })
                }
                Attributes::Text | Attributes::BinaryVector => {
                    let mut buffer = [0; 16];
                    buffer.copy_from_slice(&self.data[value_offset..value_offset + 16]);
                    let [length, cursor] = cast::<[u8; 16], [u64; 2]>(buffer);

                    Some(Data::Extent {
                        length,
                        cursor: Some(cursor),
                    })
                }
                _ => None,
            },
            None => None,
        }
    }

    /// Reads current value as a blob device, if current value is a text-buffer or binary vec,
    ///
    pub fn read_as_blob(
        &self,
        interner: &Interner,
        blob_device: &Cursor<Vec<u8>>,
    ) -> Option<BlobDevice> {
        match self.read_value(interner, blob_device) {
            Some(value) => {
                let name = self.name(interner).unwrap_or_default();
                let symbol = self.symbol(interner).unwrap_or_default();
                let address = format!("{name}::{symbol}");

                match value {
                    Value::TextBuffer(text_buffer) => Some(BlobDevice::new(
                        address,
                        Cursor::new(text_buffer.as_bytes().to_vec()),
                    )),
                    Value::BinaryVector(binary) => {
                        Some(BlobDevice::new(address, Cursor::new(binary)))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Returns true if this frame stores extent data,
    ///
    pub fn is_extent(&self) -> bool {
        match self.attribute() {
            Some(attr) => match attr {
                Attributes::Text | Attributes::BinaryVector => true,
                _ => false,
            },
            None => false,
        }
    }

    /// Returns the frame, using an entity for parity bits,
    ///
    /// Only relevant for add/define frames,
    ///
    pub fn with_parity(&self, entity: Entity) -> Self {
        let mut clone = self.clone();

        let id = entity.id();
        let gen = entity.gen().id() as u32;
        let parity = cast::<[u32; 2], [u8; 8]>([id, gen]);

        clone.data[56..].copy_from_slice(&parity);
        clone
    }

    /// Sets the parity bits for this frame,
    ///
    pub fn set_parity(&mut self, entity: Entity) {
        let id = entity.id();
        let gen = entity.gen().id() as u32;
        let parity = cast::<[u32; 2], [u8; 8]>([id, gen]);

        self.data[56..].copy_from_slice(&parity);
    }

    /// Returns the data portion of the frame
    ///
    pub fn data(&self) -> [u8; 63] {
        let mut buf = [0; 63];
        buf.copy_from_slice(&self.data[1..]);
        buf
    }

    /// Returns the op byte
    ///
    pub fn op(&self) -> u8 {
        self.data[0]
    }

    /// Returns the underlying bytes
    ///
    pub fn bytes(&self) -> &[u8; 64] {
        &self.data
    }

    /// Creates a cursor w/ inner data
    ///
    fn cursor(&self) -> Cursor<[u8; 64]> {
        Cursor::new(self.data)
    }
}

impl From<Cursor<[u8; 64]>> for Frame {
    fn from(data: Cursor<[u8; 64]>) -> Self {
        Self {
            data: data.into_inner(),
        }
    }
}

impl From<[u8; 64]> for Frame {
    fn from(data: [u8; 64]) -> Self {
        Self { data }
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self { data: [0; 64] }
    }
}

/// Interim structure for building a new frame
///
pub struct FrameBuilder {
    cursor: Cursor<[u8; 64]>,
    value: Option<Value>,
}

impl Default for FrameBuilder {
    fn default() -> Self {
        Self {
            cursor: Frame::default().cursor(),
            value: None,
        }
    }
}

impl Into<Frame> for FrameBuilder {
    fn into(self) -> Frame {
        self.cursor.into()
    }
}

impl FrameBuilder {
    /// Write value sets the current value the frame builder is adding,
    ///
    pub fn write_value(
        &mut self,
        value: &Value,
        blob: &mut (impl Read + Write + Seek + Clone),
    ) -> Result<usize, std::io::Error> {
        self.value = Some(value.clone());
        self.write(value, Some(blob))
    }

    /// Writes data to the frame,
    ///
    /// If successful returns the bytes written, otherwise returns an error.
    ///
    pub fn write(
        &mut self,
        data: impl Into<Data>,
        blob: Option<&mut (impl Read + Write + Seek + Clone)>,
    ) -> Result<usize, std::io::Error> {
        let data: Data = data.into();
        match data {
            // u64 followed by entropy bytes
            Data::Length(len) => self
                .cursor
                .write(&cast::<[u64; 2], [u8; 16]>([len as u64, Self::entropy()])),
            // 8 0's followed by entropy bytes
            Data::Entropy => self
                .cursor
                .write(&cast::<[u64; 2], [u8; 16]>([0, Self::entropy()])),
            // The first byte of the frame is always an operation
            Data::Operation(op) => self.cursor.write(&[op]),
            // After 1-2 identifiers, 1 byte identifies the value type the frame
            // represents
            Data::Value(val) => self.cursor.write(&[val]),
            // Inlined data is always at the end, so padding is not required
            Data::InlineFalse => self.cursor.write(&[0x00]),
            Data::InlineTrue => self.cursor.write(&[0x01]),
            // Skip writing,
            Data::InlineEmpty => Ok(0),
            // For actual type literals, this will always be a 16-byte array
            Data::Inline { data } => self.cursor.write(&data),
            // Interned data can be in the middle and end of the frame,
            // so it's important this gets padded
            Data::Interned { key } => self
                .cursor
                .write(&cast::<[u64; 2], [u8; 16]>([key, Self::entropy()])),
            // An extent will never be in the middle of the frame,
            // So it does not require any padding.
            // However, extents require a write to a blob to figure out the cursor pos
            // So the if the cursor is not set, pad with u64::MAX
            // All of these details should be handled by the encoder.
            Data::Extent { length, cursor } => match cursor {
                Some(cursor) => self
                    .cursor
                    .write(&cast::<[u64; 2], [u8; 16]>([length, cursor])),
                None => match (blob, self.value.as_ref()) {
                    (Some(blob), Some(value)) => {
                        let data = Data::parse_blob(value.clone(), blob).expect("blob is parsed");

                        self.write(data, None::<&mut Cursor<Vec<u8>>>)
                    }
                    _ => self
                        .cursor
                        .write(&cast::<[u64; 2], [u8; 16]>([length, u64::MAX])),
                },
            },
            // Frame extents will be constructed from regular extents,
            Data::FrameExtent {
                start,
                end,
                cursor,
                length,
            } => self
                .cursor
                .write(&cast::<[u64; 4], [u8; 32]>([start, end, cursor, length])),
        }
    }

    /// Returns some entropy,
    ///
    fn entropy() -> u64 {
        rand::thread_rng().next_u64()
    }
}

#[test]
fn test_frame() {
    let mut frame = Frame::default();

    // Test writing to the frame
    let mut cursor = frame.cursor();
    let written = std::io::Write::write(&mut cursor, &[0x01, 0x02, 0x03]).expect("can write");
    assert_eq!(written, 3);
    assert_eq!(cursor.position(), 3);

    // Test reading from the frame
    frame = cursor.into();
    let mut cursor = frame.cursor();
    let mut buf = [0; 3];
    std::io::Read::read_exact(&mut cursor, &mut buf).expect("can read");
    assert_eq!(buf, [0x01, 0x02, 0x03]);
}

/// Tests frame building and decoding
///
#[test]
fn test_frame_building() {
    let mut interner = HashMap::<u64, String>::new();

    [
        "call", "count", "counter", "label", "triangle", "pair", "single",
    ]
    .map(|i| Value::Symbol(i.to_string()))
    .iter()
    .for_each(|s| {
        if let (Value::Reference(key), Value::Symbol(symbol)) = (s.to_ref(), s) {
            interner.insert(key, symbol.to_string());
        } else {
            unreachable!()
        }
    });

    let interner = Interner::from(interner);

    let mut blob = Cursor::new(vec![]);

    let frame = Frame::start_block("call", "counter");
    assert_eq!(frame.name(&interner), Some("call".to_string()));
    assert_eq!(frame.symbol(&interner), Some("counter".to_string()));

    let frame = Frame::add(
        "count",
        &Value::TextBuffer("hello world".to_string()),
        &mut blob,
    );
    assert_eq!(frame.name(&interner), Some("count".to_string()));
    assert_eq!(frame.symbol(&interner), None);
    assert_eq!(frame.attribute(), Some(Attributes::Text));
    assert_eq!(
        frame.read_value(&interner, &blob),
        Some(Value::TextBuffer("hello world".to_string()))
    );

    let frame = Frame::define(
        "count",
        "label",
        &Value::TextBuffer("hello world".to_string()),
        &mut blob,
    );
    assert_eq!(frame.name(&interner), Some("count".to_string()));
    assert_eq!(frame.symbol(&interner), Some("label".to_string()));
    assert_eq!(frame.attribute(), Some(Attributes::Text));
    assert_eq!(
        frame.read_value(&interner, &blob),
        Some(Value::TextBuffer("hello world".to_string()))
    );

    let frame = Frame::define("count", "label", &Value::Empty, &mut blob);
    assert_eq!(frame.name(&interner), Some("count".to_string()));
    assert_eq!(frame.symbol(&interner), Some("label".to_string()));
    assert_eq!(frame.attribute(), Some(Attributes::Empty));

    let frame = Frame::define("count", "triangle", &Value::IntRange(300, 10, 4), &mut blob);
    assert_eq!(frame.name(&interner), Some("count".to_string()));
    assert_eq!(frame.symbol(&interner), Some("triangle".to_string()));
    assert_eq!(frame.attribute(), Some(Attributes::IntRange));

    let frame = Frame::define("count", "pair", &Value::IntPair(300, 10), &mut blob);
    assert_eq!(frame.name(&interner), Some("count".to_string()));
    assert_eq!(frame.symbol(&interner), Some("pair".to_string()));
    assert_eq!(frame.attribute(), Some(Attributes::IntPair));

    let frame = Frame::define("count", "single", &Value::Int(300), &mut blob);
    assert_eq!(frame.name(&interner), Some("count".to_string()));
    assert_eq!(frame.symbol(&interner), Some("single".to_string()));
    assert_eq!(frame.attribute(), Some(Attributes::Int));

    let world = <specs::World as specs::WorldExt>::new();
    specs::WorldExt::entities(&world).create();
    let parity_test = specs::WorldExt::entities(&world).create();
    let frame_with_parity = frame.with_parity(parity_test);

    eprintln!("{:#}", frame_with_parity);
    Frame::end_block();
}
