use super::{frame::Frame, BlobDevice, Interner, WireObject};
use atlier::system::Value;
use specs::World;
use std::{collections::BTreeMap, io::{Cursor, Seek, Write, Read}, ops::Range};

/// Frame index
/// 
pub type FrameIndex = BTreeMap<String, Vec<Range<usize>>>;

/// Struct for encoding resources into frames,
///
#[derive(Debug)]
pub struct Encoder<BlobImpl = Cursor<Vec<u8>>> 
where
    BlobImpl: Read + Write + Seek + Clone + Default
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
    BlobImpl: Read + Write + Seek + Clone + Default
{
    /// Returns a new encoder /w a blob_device
    ///
    pub fn new_with(blob_device: impl Into<BlobImpl>) -> Self {
        Self {
            interner: Interner::default(),
            blob_device: blob_device.into(),
            frames: vec![],
            frame_index: BTreeMap::new(),
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
        T: WireObject
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

#[test]
#[tracing_test::traced_test]
fn test_encoder() {
    use crate::Block;
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
}
