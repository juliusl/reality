use super::{
    frame::{ExtensionToken, Frame},
    BlobDevice, Interner, WireObject,
};
use atlier::system::Value;
use specs::{Entity, World};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Seek, Write},
    ops::Range,
};

/// Frame index
///
pub type FrameIndex = BTreeMap<String, Vec<Range<usize>>>;

/// Struct for encoding resources into frames,
///
#[derive(Debug)]
pub struct Encoder<BlobImpl = Cursor<Vec<u8>>>
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// String interner for storing identifiers and complexes
    ///
    /// Can be converted into frames,
    ///
    pub interner: Interner,
    /// Cursor to a blob device for writing/reading extent data types,
    ///
    pub blob_device: BlobImpl,
    /// Frames that have been encoded,
    ///
    pub frames: Vec<Frame>,
    /// Index for labeling ranges of frames,
    ///
    /// Ex. For encoding blocks, uses the key format `{name} {symbol}`,
    ///
    pub frame_index: FrameIndex,
    /// This is the last enttiy whose component was encoded
    ///
    pub last_entity: Option<Entity>,
}

impl<BlobImpl> Default for Encoder<BlobImpl>
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    fn default() -> Self {
        Self::new_with(BlobImpl::default())
    }
}

impl Encoder {
    /// Returns a new encoder w/ an empty in-memory blob device
    ///
    pub fn new() -> Self {
        Self::new_with(Cursor::<Vec<u8>>::new(vec![]))
    }

    /// Returns a blob device using the current cursor state
    ///
    pub fn blob_device(&self, address: impl AsRef<str>) -> BlobDevice {
        BlobDevice::existing(address, &self.blob_device.clone())
    }
}

impl<BlobImpl> Encoder<BlobImpl>
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// Returns a new encoder /w a blob_device
    ///
    pub fn new_with(blob_device: impl Into<BlobImpl>) -> Self {
        Self {
            interner: Interner::default(),
            blob_device: blob_device.into(),
            frames: vec![],
            frame_index: BTreeMap::new(),
            last_entity: None,
        }
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
    pub fn frame_index(&self) -> &FrameIndex {
        &self.frame_index
    }

    /// Encodes a wire object,
    ///
    pub fn encode<T>(&mut self, obj: &T, world: &World)
    where
        T: WireObject,
    {
        obj.encode(world, self);
    }

    /// Interns a vector of identifiers,
    ///
    pub fn intern_identifiers(&mut self, identifiers: Vec<String>) {
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

    /// Clears the protocol,
    ///
    pub fn clear(&mut self) {
        self.blob_device = BlobImpl::default();
        self.frame_index.clear();
        self.frames.clear();
        self.interner = Interner::default();
    }
}

impl<BlobImpl> Encoder<BlobImpl>
where
    BlobImpl: std::io::Read + std::io::Write + std::io::Seek + Clone + Default,
{
    /// Starts an extension frame w/ this encoder,
    ///
    /// Returns an ExtensionToken, when the token is dropped, the extension frame will be encoded,
    /// w/ the number of frames that were encoded before it was dropped
    ///
    pub fn start_extension<'a>(
        &'a mut self,
        namespace: impl AsRef<str>,
        symbol: impl AsRef<str>,
    ) -> ExtensionToken<'a, BlobImpl>
    where
        BlobImpl: Read + Write + Seek + Clone + Default,
    {
        self.interner.add_ident(namespace.as_ref());
        self.interner.add_ident(symbol.as_ref());

        ExtensionToken::<'a, BlobImpl>::new(namespace, symbol, self)
    }

    /// Adds a stable value frame, returns the frame for final configuration,
    ///
    pub fn add_value(&mut self, name: impl AsRef<str>, value: impl Into<Value>) -> &mut Frame {
        self.interner.add_ident(name.as_ref());

        let value: Value = value.into();

        match &value {
            Value::Symbol(symbol) => {
                self.interner.add_ident(symbol);
            }
            Value::Complex(complex) => {
                for s in complex.iter() {
                    self.interner.add_ident(s);
                }
            }
            _ => {}
        }

        self.frames.push(Frame::add(
            name.as_ref(),
            &value.into(),
            &mut self.blob_device,
        ));
        self.frames.last_mut().expect("should exist, just added")
    }

    /// Defines a transient property value, returns the frame for final configuration,
    ///
    pub fn define_property(
        &mut self,
        name: impl AsRef<str>,
        property: impl AsRef<str>,
        value: impl Into<Value>,
    ) -> &mut Frame {
        self.interner.add_ident(name.as_ref());
        self.interner.add_ident(property.as_ref());

        let value: Value = value.into();

        match &value {
            Value::Symbol(symbol) => {
                self.interner.add_ident(symbol);
            }
            Value::Complex(complex) => {
                for s in complex.iter() {
                    self.interner.add_ident(s);
                }
            }
            _ => {}
        }

        self.frames.push(Frame::define(
            name.as_ref(),
            property.as_ref(),
            &value.into(),
            &mut self.blob_device,
        ));

        self.frames.last_mut().expect("should exist, just added")
    }

    /// Adds a stable symbol frame,
    ///
    /// Symbol values are interned so they are sent w/ the control device and centralized,
    ///
    pub fn add_symbol(&mut self, name: impl AsRef<str>, symbol: impl AsRef<str>) -> &mut Frame {
        self.add_value(name.as_ref(), Value::Symbol(symbol.as_ref().to_string()))
    }

    /// Adds a stable text-buffer frame,
    ///
    /// Text buffer values are UTF8 strings stored in the blob-device, and are transported last,
    ///
    pub fn add_text(&mut self, name: impl AsRef<str>, text_buffer: impl AsRef<str>) -> &mut Frame {
        self.add_value(
            name.as_ref(),
            Value::TextBuffer(text_buffer.as_ref().to_string()),
        )
    }

    /// Adds a stable binary frame,
    ///
    /// Binary values are stored in the blob-device, and are transported last,
    ///
    pub fn add_binary(&mut self, name: impl AsRef<str>, bytes: impl Into<Vec<u8>>) -> &mut Frame {
        self.add_value(name.as_ref(), Value::BinaryVector(bytes.into()))
    }

    /// Adds a stable int frame,
    ///
    /// Int values are stored inline and transported w/ the frame,
    ///
    pub fn add_int(&mut self, name: impl AsRef<str>, int: impl Into<i32>) -> &mut Frame {
        self.add_value(name.as_ref(), Value::Int(int.into()))
    }

    /// Adds a stable float frame,
    ///
    /// Float values are stored inline and transported w/ the frame,
    ///
    pub fn add_float(&mut self, name: impl AsRef<str>, float: impl Into<f32>) -> &mut Frame {
        self.add_value(name.as_ref(), Value::Float(float.into()))
    }

    /// Defines a transient symbol property frame and add's to the encoder,
    ///
    /// Symbol values are interned so they are sent w/ the control device and centralized,
    ///
    /// Transient values are represented by two identifiers and can be interpreted as transient properties of a stable attribute,
    ///
    pub fn define_symbol(
        &mut self,
        name: impl AsRef<str>,
        property: impl AsRef<str>,
        symbol: impl AsRef<str>,
    ) -> &mut Frame {
        self.interner.add_ident(symbol.as_ref());

        self.define_property(name, property, Value::Symbol(symbol.as_ref().to_string()))
    }

    /// Defines a transient text buffer property frame and add's to the encoder,
    ///
    /// Text buffer values are UTF8 strings stored in the blob-device, and are transported last,
    ///
    /// Transient values are represented by two identifiers and can be interpreted as transient properties of a stable attribute,
    ///
    pub fn define_text(
        &mut self,
        name: impl AsRef<str>,
        property: impl AsRef<str>,
        text_buffer: impl AsRef<str>,
    ) -> &mut Frame {
        self.define_property(
            name,
            property,
            Value::TextBuffer(text_buffer.as_ref().to_string()),
        )
    }

    /// Defines a transient binary property frame and add's to the encoder,
    ///
    /// Binary values are stored in the blob-device, and are transported last,
    ///
    /// Transient values are represented by two identifiers and can be interpreted as transient properties of a stable attribute,
    ///
    pub fn define_binary(
        &mut self,
        name: impl AsRef<str>,
        property: impl AsRef<str>,
        bytes: impl Into<Vec<u8>>,
    ) -> &mut Frame {
        self.define_property(name, property, Value::BinaryVector(bytes.into()))
    }

    /// Defines a transient int property frame and add's to the encoder,
    ///
    /// Int values are stored inline and transported w/ the frame,
    ///
    /// Transient values are represented by two identifiers and can be interpreted as transient properties of a stable attribute,
    ///
    pub fn define_int(
        &mut self,
        name: impl AsRef<str>,
        property: impl AsRef<str>,
        int: impl Into<i32>,
    ) -> &mut Frame {
        self.define_property(name, property, Value::Int(int.into()))
    }

    /// Defines a transient float property frame and add's to the encoder,
    ///
    /// Int values are stored inline and transported w/ the frame,
    ///
    /// Transient values are represented by two identifiers and can be interpreted as transient properties of a stable attribute,
    ///
    pub fn define_float(
        &mut self,
        name: impl AsRef<str>,
        property: impl AsRef<str>,
        float: impl Into<f32>,
    ) -> &mut Frame {
        self.define_property(name, property, Value::Float(float.into()))
    }
}

#[test]
#[tracing_test::traced_test]
fn test_encoder() {
    use crate::Block;
    use crate::Keywords;
    use specs::WorldExt;
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

    ``` guest
    + address .text localhost
    ```
    "#;

    let mut parser = crate::Parser::new().parse(content);
    parser.evaluate_stack();

    let world = parser.world();
    let mut encoder = Encoder::new();
    encoder.encode(parser.get_block("call", "guest"), &world);
    encoder.encode(parser.get_block("call", "host"), &world);
    encoder.encode(parser.get_block("test", "host"), &world);
    encoder.encode(parser.get_block("", "guest"), &world);
    encoder.encode(parser.get_block("", "guest"), &world);
    encoder.encode(parser.root(), &world);

    let index = Block::build_index(&encoder.interner(), encoder.frames_slice());

    println!("{:#?}", index);

    // Test `call guest`
    let value = encoder.frames[1]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::TextBuffer("localhost".to_string()));

    let value = encoder.frames[2]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::Bool(true));

    let value = encoder.frames[3]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::TextBuffer("api/test2".to_string()));

    // Test `call host`
    let value = encoder.frames[6]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::TextBuffer("localhost".to_string()));

    let value = encoder.frames[7]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::Bool(true));

    let value = encoder.frames[8]
        .read_value(&encoder.interner, &mut encoder.blob_device)
        .expect("can read");
    assert_eq!(value, Value::TextBuffer("api/test".to_string()));

    for f in encoder.frames_slice() {
        event!(Level::TRACE, "{:#}", f);
    }

    let control_device = crate::wire::ControlDevice::new(encoder.interner.clone());
    // This is the size in memory
    event!(
        Level::TRACE,
        "total memory size      : {} bytes",
        content.len()
    );
    // When a string is serialized, this should be the size of that message w/ utf8 encoding
    event!(
        Level::TRACE,
        "total utf8 size        : {} bytes",
        content.chars().count() * 4
    );
    event!(
        Level::TRACE,
        "control_dev size       : {} frames {} total bytes",
        control_device.len(),
        control_device.size()
    );
    event!(
        Level::TRACE,
        "encoded block size     : {} frames {} total bytes",
        &encoder.frames_slice().len(),
        &encoder.frames_slice().len() * 64
    );
    event!(
        Level::TRACE,
        "total blob device size : {} bytes",
        &encoder.blob_device("").size()
    );

    let mut encoder = Encoder::new();
    let test_entity = world.entities().create();
    {
        encoder
            .add_symbol("test_symbol", "symbol_value")
            .set_parity(test_entity);
    }
    encoder.add_int("test_int", 10);
    encoder.add_float("name", 10.99);

    assert_eq!(encoder.frames[0].keyword(), Keywords::Add);
    assert_eq!(
        encoder.frames[0].read_value(&encoder.interner, &encoder.blob_device),
        Some(Value::Symbol("symbol_value".to_string()))
    );
    assert_eq!(encoder.frames[0].get_entity(&world, true), test_entity);

    //
    {
        let mut token = encoder.start_extension("test_extension", "object");

        token.as_mut().add_symbol("name", "hello");
        token.as_mut().add_int("name", 10);
    }

    assert_eq!(encoder.frames[3].keyword(), Keywords::Extension);
    assert_eq!(encoder.frames[3].frame_len(), 3);
}
