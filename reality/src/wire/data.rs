use std::io::{Write, Seek, Read};

use crate::Value;
use bytemuck::cast;
use tracing::{event, Level};

use crate::parser::{Attributes, Elements, Keywords};

/// Chunk of data
///
/// **Note** A frame is 4 chunks.
///
pub type Chunk = [u8; 16];

/// Wire data types, to use for frame encoding
///
pub enum Data {
    /// Frame length,
    /// 
    Length(usize),
    /// Random data, 
    /// 
    Entropy,
    /// Inlined false value
    ///
    /// Will be encoded as 0x00
    ///
    InlineFalse,
    /// Inlined true value
    ///
    /// Will be encoded as 0x01
    ///
    InlineTrue,
    /// Inlined empty value
    ///
    /// Will skip encoding to the frame, left as an instruction for the frame.
    ///
    InlineEmpty,
    /// Operation type
    ///
    Operation(u8),
    /// Value type
    ///
    Value(u8),
    /// Data where the value can be transported w/ the frame.
    ///
    Inline { data: Chunk },
    /// Data where a reference to the value is transported w/ the frame.
    ///
    Interned { key: u64 },
    /// Data where a cursor to find the value, is transported w/ the frame.
    ///
    Extent { length: u64, cursor: Option<u64> },
    /// Data where the value is contained within a range of frames. Once combined,
    /// the value is retrieved like a normal extent. 
    /// 
    FrameExtent { start: u64, end: u64, cursor: u64, length: u64 }
}

impl Data {
    /// Parses a text buffer or binary vector value type, and writes to the blob
    /// cursor, returns an extent to look up the value.
    ///
    pub fn parse_blob(value: Value, blob: &mut (impl Read + Write + Seek + Clone )) -> Option<Self> {
        match value {
            Value::TextBuffer(text) => {
                let cursor = blob.stream_position().ok();
                match blob.write(text.as_bytes()) {
                    Ok(written) => {
                        assert_eq!(written, text.len());
                        Some(Self::Extent {
                            length: written as u64,
                            cursor,
                        })
                    }
                    Err(err) => {
                        event!(Level::ERROR, "error writing to blob {err}");
                        None
                    }
                }
            }
            Value::BinaryVector(data) => {
                let cursor = blob.stream_position().ok();
                match blob.write(&data) {
                    Ok(written) => {
                        assert_eq!(written, data.len());
                        Some(Self::Extent {
                            length: written as u64,
                            cursor,
                        })
                    }
                    Err(err) => {
                        event!(Level::ERROR, "error writing to blob {err}");
                        None
                    }
                }
            }
            _ => None,
        }
    }
}

impl From<Attributes> for Data {
    fn from(feature: Attributes) -> Self {
        Data::Value(feature as u8)
    }
}

impl From<Keywords> for Data {
    fn from(k: Keywords) -> Self {
        Data::Operation(k as u8)
    }
}

impl From<Elements> for Data {
    fn from(ident: Elements) -> Self {
        match ident {
            Elements::Identifier(ident) => {
                if let Value::Reference(key) = Value::Symbol(ident).to_ref() {
                    Data::Interned { key }
                } else {
                    unreachable!("to_ref should always return a reference value")
                }
            },
            Elements::InlineOperator => {
                panic!("inline operator is not encoded to frame")
            }
            Elements::EscapedColon => {
                panic!("escaped colon is not encoded to frame")
            }
            Elements::Comment(_) => {
                panic!("comment element is not encoded to frame")
            }
            Elements::NewLine => {
                panic!("newline element is not encoded to frame")
            }
            Elements::Error => {
                panic!("error is not encoded to frame")
            }
        }
    }
}

impl From<&Value> for Data {
    fn from(val: &Value) -> Self {
        match val {
            Value::Empty => Data::InlineEmpty,
            Value::Bool(true) => Data::InlineTrue,
            Value::Bool(false) => Data::InlineFalse,
            Value::Int(v) => Data::Inline {
                data: cast::<[i32; 4], Chunk>([*v, 0, 0, 0]),
            },
            Value::IntPair(a, b) => Data::Inline {
                data: cast::<[i32; 4], Chunk>([*a, *b, 0, 0]),
            },
            Value::IntRange(a, b, c) => Data::Inline {
                data: cast::<[i32; 4], Chunk>([*a, *b, *c, 0]),
            },
            Value::Float(v) => Data::Inline {
                data: cast::<[f32; 4], Chunk>([*v, 0.0, 0.0, 0.0]),
            },
            Value::FloatPair(a, b) => Data::Inline {
                data: cast::<[f32; 4], Chunk>([*a, *b, 0.0, 0.0]),
            },
            Value::FloatRange(a, b, c) => Data::Inline {
                data: cast::<[f32; 4], Chunk>([*a, *b, *c, 0.0]),
            },
            Value::TextBuffer(v) => Data::Extent {
                length: v.len() as u64,
                cursor: None,
            },
            Value::BinaryVector(v) => Data::Extent {
                length: v.len() as u64,
                cursor: None,
            },
            Value::Reference(key) => Data::Interned { key: *key },
            Value::Symbol(_) => {
                if let Value::Reference(key) = val.to_ref() {
                    Data::Interned { key }
                } else {
                    unreachable!("to_ref() should never return anything but a Value::Reference")
                }
            }
            Value::Complex(_) => {
                if let Value::Reference(key) = val.to_ref() {
                    Data::Interned { key }
                } else {
                    unreachable!("to_ref() should never return anything but a Value::Reference")
                }
            }
        }
    }
}
