mod custom;
pub use custom::CustomAttribute;
pub use custom::SpecialAttribute;

mod file;
pub use file::FileDescriptor;

mod blob;
pub use blob::BlobDescriptor;

use std::collections::HashMap;
use std::sync::Arc;
use std::{collections::BTreeSet, fmt::Display, str::FromStr};

use atlier::system::{Attribute, Value};
use logos::{Lexer, Logos};
use specs::{World, WorldExt};
use tracing::{event, Level};

use crate::parser::Elements;

/// Parser for parsing attributes
///
#[derive(Default, Clone)]
pub struct AttributeParser {
    /// Entity id
    id: u32,
    /// Defaults to Value::Empty
    value: Value,
    /// Attribute name
    name: Option<String>,
    /// Transient symbol
    symbol: Option<String>,
    /// Transient value
    edit: Option<Value>,
    /// Stack of attributes
    stack: Vec<Attribute>,
    /// Custom attribute parsers
    custom_attributes: HashMap<String, CustomAttribute>,
    /// Reference to world being edited
    world: Option<Arc<World>>,
}

impl AttributeParser {
    pub fn init(mut self, content: impl AsRef<str>) -> Self {
        self.parse(content);
        self
    }

    /// Parses content, updating internal state
    ///
    pub fn parse(&mut self, content: impl AsRef<str>) -> &mut Self {
        let custom_attributes = self.custom_attributes.clone();

        let mut lexer = Attributes::lexer_with_extras(content.as_ref(), self.clone());

        while let Some(token) = lexer.next() {
            match token {
                Attributes::Error => {
                    let line = format!("{}{}", lexer.slice(), lexer.remainder());
                    event!(
                        Level::WARN,
                        "Could not parse type, checking custom attribute parsers",
                    );

                    let mut elements_lexer = Elements::lexer(&line);
                    match elements_lexer.next() {
                        Some(element) => match element {
                            Elements::AttributeType(custom_attr_type) => {
                                match custom_attributes.get(&custom_attr_type) {
                                    Some(custom_attr) => {
                                        custom_attr.parse(
                                            &mut lexer.extras, 
                                            elements_lexer.remainder()
                                        );
                                        lexer.bump(lexer.remainder().len());
                                    }
                                    None => {
                                        event!(
                                            Level::ERROR, 
                                            "Did not parse {custom_attr_type}, could not find custom attribute parser", 
                                        );
                                    }
                                }
                            }
                            _ => {
                                event!(
                                    Level::ERROR,
                                    "Did not parse {:?}, unexpected element",
                                    element
                                );
                            }
                        },
                        None => {
                            event!(
                                Level::ERROR,
                                "Did not parse attribute, no elements parsed",
                            );
                        }
                    }
                }
                _ => {
                    event!(Level::TRACE, "Parsed {:?}", token);
                }
            }
        }


        *self = lexer.extras;
        self
    }

    /// Adds a custom attribute parser and returns self,
    ///
    pub fn with_custom<C>(mut self) -> Self 
    where
        C: SpecialAttribute
    {
        let custom_attr = CustomAttribute::new::<C>();
        self.custom_attributes.insert(custom_attr.ident(), custom_attr);
        self
    }

    /// Adds a custom attribute parser,
    ///
    pub fn add_custom(&mut self, custom_attr: CustomAttribute)
    {
        self.custom_attributes.insert(custom_attr.ident(), custom_attr);
    }

    /// Returns the next attribute from the stack
    ///
    pub fn next(&mut self) -> Option<Attribute> {
        self.stack.pop()
    }

    /// Sets the id for the current parser
    ///
    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    /// Sets the current name value
    ///
    pub fn set_name(&mut self, name: impl AsRef<str>) {
        self.name = Some(name.as_ref().to_string());
    }

    /// Sets the current symbol value
    ///
    pub fn set_symbol(&mut self, symbol: impl AsRef<str>) {
        self.symbol = Some(symbol.as_ref().to_string());
    }

    /// Sets the current value for the parser
    ///
    pub fn set_value(&mut self, value: Value) {
        self.value = value;
    }

    /// Sets the current transient value for the parser
    ///
    pub fn set_edit(&mut self, value: Value) {
        self.edit = Some(value);
    }

    /// Sets the current world being edited,
    /// 
    pub fn set_world(&mut self, world: Arc<World>) {
        self.world = Some(world);
    }

    /// Returns the name,
    /// 
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    /// Returns an immutable reference to world,
    /// 
    pub fn world(&self) -> Option<&Arc<World>> {
        self.world.as_ref()
    }

    /// Defines a property for the current name,
    /// 
    /// Panics if a name is not set.
    /// 
    pub fn define(&mut self, symbol: impl AsRef<str>, value: impl Into<Value>) {
        self.set_symbol(symbol);
        self.set_edit(value.into());
        self.set_value(Value::Empty);
        self.parse_attribute();
    }

    /// Parses the current state into an attribute, pushes onto stack
    ///
    pub fn parse_attribute(&mut self) {
        let value = self.value.clone();
        let name = self.name.clone();
        let symbol = self.symbol.take();
        let edit = self.edit.take();

        match (name, symbol, value, edit) {
            (Some(name), Some(symbol), value, Some(edit)) => {
                let mut attr = Attribute::new(self.id, format!("{name}::{symbol}"), value);
                attr.edit_as(edit);
                self.stack.push(attr);
            }
            (Some(name), None, value, None) => {
                let attr = Attribute::new(self.id, name, value);
                self.stack.push(attr);
            }
            _ => {}
        }
    }

    /// Parses a value,
    ///
    /// If symbol is set, then this value will be set to edit,
    /// otherwise, value will be set
    ///
    fn parse_value(&mut self, value: Value) {
        if self.symbol.is_some() {
            self.set_edit(value);
        } else {
            self.set_value(value);
        }
    }

    /// Parses a symbol,
    ///
    /// In this context, this is either a name or symbol.
    ///
    fn parse_symbol(&mut self, symbol: String) {
        if self.name.is_none() {
            self.set_name(symbol)
        } else {
            self.set_symbol(symbol)
        }
    }
}

/// Decompose an attribute into an attribute parser
///
impl From<Attribute> for AttributeParser {
    fn from(attr: Attribute) -> Self {
        let id = attr.id;

        let name = Some(attr.name.to_string());

        let symbol = {
            if attr.is_stable() {
                None
            } else {
                attr.name
                    .split_once("::")
                    .and_then(|(_, symbol)| Some(symbol.to_string()))
            }
        };

        let value = attr.value.clone();
        let edit = attr.transient().and_then(|(_, val)| Some(val.clone()));

        Self {
            id,
            name,
            symbol,
            value,
            edit,
            stack: vec![],
            custom_attributes: HashMap::default(),
            world: None,
        }
    }
}

/// Creates a new parser and entity from world,
///
impl From<&'_ World> for AttributeParser {
    fn from(world: &'_ World) -> Self {
        let entity = world.entities().create();
        let mut parser = AttributeParser::default();
        parser.set_id(entity.id());
        parser
    }
}

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
#[derive(Logos, Debug, PartialEq, Eq)]
#[logos(extras = AttributeParser)]
pub enum Attributes {
    /// # Inlined
    ///
    /// The max attribute value that is inlined is 3 x 32 bit values. To allow for future updates,
    /// this is extended to an assumed max space of 4 x 32 bit values, or 2 x 64 bit values.
    ///
    /// This aligns to [u8; 16]
    ///
    /// Empty value attribute
    ///
    /// # Special empty values
    ///
    /// .map - This indicates that this attribute carries no values,
    ///        and only has map properties
    ///
    #[token(".empty", on_empty_attr)]
    #[token(".map", on_empty_attr)]
    Empty = 0x00,
    /// bool element parses remaining as bool
    #[token(".enable", on_bool_enable)]
    #[token(".disable", on_bool_disable)]
    #[token(".true", on_bool_enable)]
    #[token(".false", on_bool_disable)]
    #[token(".bool", on_bool_attr)]
    Bool = 0x01,
    /// int element parses remaining as i32
    #[token(".int", on_int_attr)]
    Int = 0x02,
    /// int pair element parses remaining as 2 comma-delimmited i32's
    #[token(".int_pair", on_int_pair_attr)]
    IntPair = 0x03,
    /// int range element parses remaining as 3 comma-delimitted i32's
    #[token(".int_range", on_int_range_attr)]
    IntRange = 0x04,
    /// float element parses remaining as f32
    #[token(".float", on_float_attr)]
    Float = 0x05,
    /// float pair element parses reamining as 2 comma delimitted f32's
    #[token(".float_pair", on_float_pair_attr)]
    FloatPair = 0x06,
    /// float range element parses remaining as 3 comma delimitted f32's
    #[token(".float_range", on_float_range_attr)]
    FloatRange = 0x07,

    /// # Interned
    ///
    /// The size of an interned hash value is u64, To allow for future proofing, this
    /// is doubled to align w/ the above [u8; 16].
    ///  
    /// Identifier string, that follows a strict format
    ///
    #[regex("[A-Za-z]+[A-Za-z-;._:/@#+=$0-9]*", on_identifier)]
    #[regex(".ident", on_symbol_attr)]
    Identifier = 0x08,
    /// Symbol is an attribute value that refers to an identifier,
    ///
    #[token(".symbol", on_symbol_attr)]
    Symbol = 0x09,

    /// # Extent
    ///
    /// An extent is generally a length and position. To read from an extent, given a
    /// seekable stream, you seek to position, and read `length` of bytes.
    ///
    /// This is stored as 2 x u64 integers, and aligned to [u8; 16] as the above two
    /// types.
    ///
    /// Text buffer of UTF8 characters,
    ///
    #[token(".text", on_text_attr)]
    Text = 0x0A,
    /// Binary data of u8 bytes,
    ///
    /// If stored directly in .runmd, should be a base64 encoded string.
    ///
    #[token(".bin", on_binary_vec_attr)]
    #[token(".base64", on_binary_vec_attr)]
    BinaryVector = 0x0B,
    /// Complex type,
    ///
    /// This is used to filter mapped properties.
    #[token(".complex", on_complex_attr)]
    Complex = 0x0C,

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
            // These are special attribute types
            Attributes::Error => write!(f, ".error"),
            Attributes::Identifier => write!(f, ".ident"),
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
            0x08 => Attributes::Identifier,
            0x09 => Attributes::Symbol,
            0x0A => Attributes::Text,
            0x0B => Attributes::BinaryVector,
            0x0C => Attributes::Complex,
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

fn on_identifier(lexer: &mut Lexer<Attributes>) {
    let slice = lexer.slice();
    lexer.extras.parse_symbol(slice.to_string());
}

fn on_text_attr(lexer: &mut Lexer<Attributes>) {
    let remaining = lexer.remainder().trim().to_string();

    let text_buf = Value::TextBuffer(remaining);

    lexer.extras.parse_value(text_buf);

    lexer.bump(lexer.remainder().len());
}

fn on_bool_attr(lexer: &mut Lexer<Attributes>) {
    let bool_attr = if let Some(value) = lexer.remainder().trim().parse().ok() {
        Value::Bool(value)
    } else {
        Value::Bool(false)
    };

    lexer.extras.parse_value(bool_attr);
    lexer.bump(lexer.remainder().len());
}

fn on_bool_enable(lexer: &mut Lexer<Attributes>) {
    lexer.extras.parse_value(Value::Bool(true));
}

fn on_bool_disable(lexer: &mut Lexer<Attributes>) {
    lexer.extras.parse_value(Value::Bool(false));
}

fn on_int_attr(lexer: &mut Lexer<Attributes>) {
    let int_attr = if let Some(value) = lexer.remainder().trim().parse::<i32>().ok() {
        Value::Int(value)
    } else {
        Value::Int(0)
    };

    lexer.extras.parse_value(int_attr);
    lexer.bump(lexer.remainder().len());
}

fn on_int_pair_attr(lexer: &mut Lexer<Attributes>) {
    let pair = from_comma_sep::<i32>(lexer);

    let int_pair = match (pair.get(0), pair.get(1)) {
        (Some(f0), Some(f1)) => Value::IntPair(*f0, *f1),
        _ => Value::IntPair(0, 0),
    };

    lexer.extras.parse_value(int_pair);
    lexer.bump(lexer.remainder().len());
}

fn on_int_range_attr(lexer: &mut Lexer<Attributes>) {
    let range = from_comma_sep::<i32>(lexer);

    let int_range = match (range.get(0), range.get(1), range.get(2)) {
        (Some(f0), Some(f1), Some(f2)) => Value::IntRange(*f0, *f1, *f2),
        _ => Value::IntRange(0, 0, 0),
    };

    lexer.extras.parse_value(int_range);
    lexer.bump(lexer.remainder().len());
}

fn on_float_attr(lexer: &mut Lexer<Attributes>) {
    let float = if let Some(value) = lexer.remainder().trim().parse::<f32>().ok() {
        Value::Float(value)
    } else {
        Value::Float(0.0)
    };

    lexer.extras.parse_value(float);
    lexer.bump(lexer.remainder().len());
}

fn on_float_pair_attr(lexer: &mut Lexer<Attributes>) {
    let pair = from_comma_sep::<f32>(lexer);
    let float_pair = match (pair.get(0), pair.get(1)) {
        (Some(f0), Some(f1)) => Value::FloatPair(*f0, *f1),
        _ => Value::FloatPair(0.0, 0.0),
    };

    lexer.extras.parse_value(float_pair);
    lexer.bump(lexer.remainder().len());
}

fn on_float_range_attr(lexer: &mut Lexer<Attributes>) {
    let range = from_comma_sep::<f32>(lexer);

    let float_range = match (range.get(0), range.get(1), range.get(2)) {
        (Some(f0), Some(f1), Some(f2)) => Value::FloatRange(*f0, *f1, *f2),
        _ => Value::FloatRange(0.0, 0.0, 0.0),
    };

    lexer.extras.parse_value(float_range);
    lexer.bump(lexer.remainder().len());
}

fn on_binary_vec_attr(lexer: &mut Lexer<Attributes>) {
    let binary = match base64::decode(lexer.remainder().trim()) {
        Ok(content) => Value::BinaryVector(content),
        Err(_) => Value::BinaryVector(vec![]),
    };

    lexer.extras.parse_value(binary);
    lexer.bump(lexer.remainder().len());
}

fn on_symbol_attr(lexer: &mut Lexer<Attributes>) {
    let remaining = lexer.remainder().trim().to_string();

    let symbol_val = Value::Symbol(remaining);

    lexer.extras.parse_value(symbol_val);
    lexer.bump(lexer.remainder().len());
}

fn on_empty_attr(lexer: &mut Lexer<Attributes>) {
    lexer.extras.parse_value(Value::Empty);
    lexer.bump(lexer.remainder().len());
}

fn on_complex_attr(lexer: &mut Lexer<Attributes>) {
    let idents = from_comma_sep::<String>(lexer);

    lexer
        .extras
        .parse_value(Value::Complex(BTreeSet::from_iter(idents)));
    lexer.bump(lexer.remainder().len());
}

fn from_comma_sep<T>(lexer: &mut Lexer<Attributes>) -> Vec<T>
where
    T: FromStr,
{
    lexer
        .remainder()
        .trim()
        .split(",")
        .filter_map(|i| i.trim().parse().ok())
        .collect()
}

#[test]
#[tracing_test::traced_test]
fn test_attribute_parser() {
    // Test parsing add
    let parser = AttributeParser::default();
    let mut lexer = Attributes::lexer_with_extras("name .text cool_name", parser);
    assert_eq!(lexer.next(), Some(Attributes::Identifier));
    assert_eq!(lexer.next(), Some(Attributes::Text));

    lexer.extras.parse_attribute();

    let attr = lexer.extras.next().expect("parses");
    assert_eq!(attr.name, "name");
    assert_eq!(attr.value, Value::TextBuffer("cool_name".to_string()));

    // Test parsing define
    let parser = AttributeParser::default();
    let mut lexer = Attributes::lexer_with_extras("connection name .text cool_name", parser);
    assert_eq!(lexer.next(), Some(Attributes::Identifier));
    assert_eq!(lexer.next(), Some(Attributes::Identifier));
    assert_eq!(lexer.next(), Some(Attributes::Text));

    lexer.extras.parse_attribute();

    let attr = lexer.extras.next().expect("parses");
    assert_eq!(attr.name, "connection::name");
    assert_eq!(attr.value, Value::Empty);
    assert_eq!(
        attr.transient.unwrap().1,
        Value::TextBuffer("cool_name".to_string())
    );

    // Complex Attributes

    let world = World::new();

    // Test parsing .file attribute
    let mut parser = AttributeParser::from(&world)
        .with_custom::<FileDescriptor>()
        .init("readme.md .file ./readme.md");

    let mut parsed = vec![];
    while let Some(attr) = parser.next() {
        parsed.push(attr);
    }
    eprintln!("{:#?}", parsed);

    // Test parsing .blob attribute
    let mut parser = AttributeParser::from(&world)
        .with_custom::<BlobDescriptor>()
        .init("readme.md .blob sha256:testdigest");

    let mut parsed = vec![];
    while let Some(attr) = parser.next() {
        parsed.push(attr);
    }
    eprintln!("{:#?}", parsed);

    AttributeParser::default()
        .init("custom .custom-attr test custom attr input");
    assert!(logs_contain(
        "Could not parse type, checking custom attribute parsers"
    ));

    let mut parser = AttributeParser::default()
        .with_custom::<TestCustomAttr>()
        .init("custom .custom-attr test custom attr input");

    parser.parse_attribute();
    
    assert_eq!(parser.next(), Some(Attribute::new(0, "custom", Value::Empty)));
}

struct TestCustomAttr();

impl SpecialAttribute for TestCustomAttr {
    fn ident() -> &'static str {
        "custom-attr"
    }

    fn parse(parser: &mut AttributeParser, _: String) {
        parser.set_value(Value::Empty);
    }
}
