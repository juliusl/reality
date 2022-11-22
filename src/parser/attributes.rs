use std::fmt::Display;
use crate::Value;
use logos::Logos;

mod custom;
pub use custom::CustomAttribute;
pub use custom::SpecialAttribute;

mod cache;
pub use cache::Cache;

mod file;
pub use file::File;

mod blob;
pub use blob::BlobDescriptor;

mod parser;
pub use parser::AttributeParser;

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
#[derive(Logos, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[logos(extras = AttributeParser)]
pub enum Attributes {
    /// Empty value
    /// 
    #[token(".empty", parser::on_empty_attr)]
    Empty = 0x00,
    ///Bool element parses remaining as bool
    /// 
    #[token(".enable", parser::on_bool_enable)]
    #[token(".disable", parser::on_bool_disable)]
    #[token(".true", parser::on_bool_enable)]
    #[token(".false", parser::on_bool_disable)]
    #[token(".bool", parser::on_bool_attr)]
    Bool = 0x01,
    /// Int element parses remaining as i32
    /// 
    #[token(".int", parser::on_int_attr)]
    Int = 0x02,
    /// Int pair element parses remaining as 2 comma-delimmited i32's
    /// 
    #[token(".int_pair", parser::on_int_pair_attr)]
    IntPair = 0x03,
    /// Int range element parses remaining as 3 comma-delimitted i32's
    /// 
    #[token(".int_range", parser::on_int_range_attr)]
    IntRange = 0x04,
    /// Float element parses remaining as f32
    /// 
    #[token(".float", parser::on_float_attr)]
    Float = 0x05,
    /// Float pair element parses reamining as 2 comma delimitted f32's
    /// 
    #[token(".float_pair", parser::on_float_pair_attr)]
    FloatPair = 0x06,
    /// Float range element parses remaining as 3 comma delimitted f32's
    /// 
    #[token(".float_range", parser::on_float_range_attr)]
    FloatRange = 0x07,
    /// Symbol is an attribute value that refers to an identifier,
    /// 
    #[token(".symbol", parser::on_symbol_attr)]
    Symbol = 0x08,
    /// Complex type, this is used to filter mapped attribute properties
    /// 
    #[token(".complex", parser::on_complex_attr)]
    Complex = 0x09,

    /// Text buffer of UTF8 characters,
    ///
    #[token(".text", parser::on_text_attr)]
    Text = 0x0A,
    /// Binary data of u8 bytes,
    ///
    /// If stored directly in .runmd, should be a base64 encoded string.
    ///
    #[token(".bin", parser::on_binary_vec_attr)]
    #[token(".base64", parser::on_binary_vec_attr)]
    BinaryVector = 0x0B,

    /// Bumps the parser until `>` is found
    /// 
    #[token("<", parser::on_comment_start)]
    Comment = 0xF0,    
    /// Identifier string, that follows a strict format
    ///
    #[regex("[A-Za-z]+[A-Za-z-;._:/@#+=$0-9]*", parser::on_identifier)]
    Identifier = 0xF1,

    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\n\f]+", logos::skip)]
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
            _ => Attributes::Error,
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
