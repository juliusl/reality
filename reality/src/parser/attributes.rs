use crate::Value;

mod custom;
pub use custom::CustomAttribute;
pub use custom::AttributeType;

mod parser;
pub use parser::AttributeParser;

mod storage_target;
pub use storage_target::StorageTarget;

mod container;
pub use container::Container;

use std::fmt::Display;

/// Enumeration of value types that parse into an attribute,
///
/// # Value Types
/// There are three categories of values, `Inline`, `Interned`, and `Extent`.
///
/// * `Inline` - These values are small enough to be directly on the wire protocol.
/// * `Interned` - These values are reused, so can be transformed into a uniform
///                byte value, and used to lookup the actual value against storage.
/// * `Extent` - These values are not consistent in length or alpha, so they must be stored
///              as BLOB data. An extent is a data structure that can be used to locate
///              the actual data.
///
/// # Formatting
/// An attribute consists of,
/// 1) 1-2 idents, (name, symbol),
/// 2) attribute type (.<ident>)
/// 3) attribute value
///
/// ex. name        .symbol attr_name
/// ex. custom name .symbol attr_name
///
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Attributes {
    /// Empty value
    /// 
    Empty = 0x00,
    ///Bool element parses remaining as bool
    /// 
    Bool = 0x01,
    /// Int element parses remaining as i32
    /// 
    Int = 0x02,
    /// Int pair element parses remaining as 2 comma-delimmited i32's
    /// 
    IntPair = 0x03,
    /// Int range element parses remaining as 3 comma-delimitted i32's
    /// 
    IntRange = 0x04,
    /// Float element parses remaining as f32
    /// 
    Float = 0x05,
    /// Float pair element parses reamining as 2 comma delimitted f32's
    /// 
    FloatPair = 0x06,
    /// Float range element parses remaining as 3 comma delimitted f32's
    /// 
    FloatRange = 0x07,
    /// Symbol is an attribute value that refers to an identifier,
    /// 
    Symbol = 0x08,
    /// Complex type, this is used to filter mapped attribute properties
    /// 
    Complex = 0x09,

    /// Text buffer of UTF8 characters,
    ///
    Text = 0x0A,
    /// Binary data of u8 bytes,
    ///
    /// If stored directly in .runmd, should be a base64 encoded string.
    ///
    BinaryVector = 0x0B,

    /// Bumps the parser until `>` is found
    /// 
    Comment = 0xF0,    
    /// Identifier string, that follows a strict format
    ///
    Identifier = 0xF1,

    Error = 0xFF,
}

impl Display for Attributes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // These are
            Attributes::Empty => write!(f, ".empty"),
            Attributes::Bool => write!(f, ".bool"),
            Attributes::Int => write!(f, ".int"),
            Attributes::IntPair => write!(f, ".int_pair"),
            Attributes::IntRange => write!(f, ".int_range"),
            Attributes::Float => write!(f, ".float"),
            Attributes::FloatPair => write!(f, ".float_pair"),
            Attributes::FloatRange => write!(f, ".float_range"),
            Attributes::Symbol => write!(f, ".symbol"),
            Attributes::Text => write!(f, ".text"),
            Attributes::BinaryVector => write!(f, ".bin"),
            Attributes::Complex => write!(f, ".complex"),
            _ => {
                Ok(())
            }
        }
    }
}

impl From<u8> for Attributes {
    fn from(c: u8) -> Self {
        match c {
            0x00 => Attributes::Empty,
            0x01 => Attributes::Bool,
            0x02 => Attributes::Int,
            0x03 => Attributes::IntPair,
            0x04 => Attributes::IntRange,
            0x05 => Attributes::Float,
            0x06 => Attributes::FloatPair,
            0x07 => Attributes::FloatRange,
            0x08 => Attributes::Symbol,
            0x09 => Attributes::Complex,
            0x0A => Attributes::Text,
            0x0B => Attributes::BinaryVector,
            0xF0 => Attributes::Comment,
            0xF1 => Attributes::Identifier,
            _ => Attributes::Error
        }
    }
}

impl From<&Value> for Attributes {
    fn from(v: &Value) -> Self {
        match v {
            Value::Empty => Attributes::Empty,
            Value::Bool(_) => Attributes::Bool,
            Value::TextBuffer(_) => Attributes::Text,
            Value::Int(_) => Attributes::Int,
            Value::IntPair(_, _) => Attributes::IntPair,
            Value::IntRange(_, _, _) => Attributes::IntRange,
            Value::Float(_) => Attributes::Float,
            Value::FloatPair(_, _) => Attributes::FloatPair,
            Value::FloatRange(_, _, _) => Attributes::FloatRange,
            Value::BinaryVector(_) => Attributes::BinaryVector,
            Value::Reference(_) => {
                unimplemented!("transforming value reference to Attributes is not supported")
            }
            Value::Symbol(_) => Attributes::Symbol,
            Value::Complex(_) => Attributes::Complex,
        }
    }
}
