use std::collections::BTreeSet;
use std::str::FromStr;
use std::{collections::HashMap, sync::Arc};

use atlier::system::{Value, Attribute};
use logos::Lexer;
use logos::Logos;
use specs::World;
use tracing::event;
use tracing::Level;

use crate::SpecialAttribute;
use crate::parser::Elements;

use super::Attributes;
use super::CustomAttribute;

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
    /// The parsed stable attribute
    parsed: Option<Attribute>,
    /// Stack of transient attribute properties
    properties: Vec<Attribute>,
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
                        Some(Elements::AttributeType(custom_attr_type)) => {
                            let mut input = elements_lexer.remainder().trim();

                            if let Some(Elements::Comment) = elements_lexer.next() {
                                input = elements_lexer.remainder().trim();
                            }
                            
                            if lexer.extras.name().is_none() {
                                lexer.extras.set_name(custom_attr_type.to_string());
                                lexer.extras.set_value(Value::Symbol(input.to_string()));
                            }

                            match custom_attributes.get(&custom_attr_type) {
                                Some(custom_attr) => {
                                    custom_attr.parse(
                                        &mut lexer.extras, 
                                        input.to_string()
                                    );
                                }
                                None => {
                                    // TODO: Add missing_custom_attribute_type
                                    event!(
                                        Level::ERROR, 
                                        "Did not parse {custom_attr_type}, could not find custom attribute parser", 
                                    );
                                }
                            }
                            lexer.bump(lexer.remainder().len());
                        },
                        _ => {
                            event!(
                                Level::ERROR,
                                "Did not parse, unexpected element"
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

        self.parse_attribute();
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
    pub fn add_custom(&mut self, custom_attr: impl Into<CustomAttribute>)
    {
        let custom_attr = custom_attr.into();
        self.custom_attributes.insert(custom_attr.ident(), custom_attr);
    }

    /// Returns the next attribute from the stack
    ///
    pub fn next(&mut self) -> Option<Attribute> {
        if !self.properties.is_empty() {
            self.properties.pop()
        } else {
            self.parsed.take()
        }
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
    pub fn set_value(&mut self, value: impl Into<Value>) {
        self.value = value.into();
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

    /// Returns the symbol,
    /// 
    pub fn symbol(&self) -> Option<&String> {
        self.symbol.as_ref()
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

    /// Defines the current attribute,
    /// 
    /// Implements the `add` keyword
    /// 
    pub fn add(&mut self, name: impl AsRef<str>, value: impl Into<Value>) {
        self.set_name(name);
        self.set_value(value.into());
        self.parse_attribute();
    }

    /// Parses the current state into an attribute, pushes onto stack
    ///
    fn parse_attribute(&mut self) {
        let value = self.value.clone();
        let name = self.name.clone();
        let symbol = self.symbol.take();
        let edit = self.edit.take();

        match (name, symbol, value, edit) {
            (Some(name), Some(symbol), value, Some(edit)) => {
                let mut attr = Attribute::new(self.id, format!("{name}::{symbol}"), value);
                attr.edit_as(edit);
                self.properties.push(attr);
            }
            (Some(name), None, value, None) => {
                let attr = Attribute::new(self.id, name, value);
                if self.parsed.is_some() {
                    event!(Level::DEBUG, "Replacing parsed attribute")
                }
                self.parsed = Some(attr);
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

pub fn on_identifier(lexer: &mut Lexer<Attributes>) {
    let slice = lexer.slice();
    lexer.extras.parse_symbol(slice.to_string());
}

pub fn on_comment_start(lexer: &mut Lexer<Attributes>) {
    let end_pos = lexer.remainder()
        .lines()
        .take(1)
        .next()
        .and_then(|s| s.find(">"))
        .expect("Didn't find a closing `>`");
    
    lexer.bump(end_pos + 1);
}

pub fn on_text_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);

    let remaining = lexer.remainder().trim().to_string();

    let text_buf = Value::TextBuffer(remaining);

    lexer.extras.parse_value(text_buf);

    lexer.bump(lexer.remainder().len());
}

pub fn on_bool_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);
    let bool_attr = if let Some(value) = lexer.remainder().trim().parse().ok() {
        Value::Bool(value)
    } else {
        Value::Bool(false)
    };

    lexer.extras.parse_value(bool_attr);
    lexer.bump(lexer.remainder().len());
}

pub fn on_bool_enable(lexer: &mut Lexer<Attributes>) {
    lexer.extras.parse_value(Value::Bool(true));
}

pub fn on_bool_disable(lexer: &mut Lexer<Attributes>) {
    lexer.extras.parse_value(Value::Bool(false));
}

pub fn on_int_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);
    
    let int_attr = if let Some(value) = lexer.remainder().trim().parse::<i32>().ok() {
        Value::Int(value)
    } else {
        Value::Int(0)
    };

    lexer.extras.parse_value(int_attr);
    lexer.bump(lexer.remainder().len());
}

pub fn on_int_pair_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);

    let pair = from_comma_sep::<i32>(lexer);

    let int_pair = match (pair.get(0), pair.get(1)) {
        (Some(f0), Some(f1)) => Value::IntPair(*f0, *f1),
        _ => Value::IntPair(0, 0),
    };

    lexer.extras.parse_value(int_pair);
    lexer.bump(lexer.remainder().len());
}

pub fn on_int_range_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);

    let range = from_comma_sep::<i32>(lexer);

    let int_range = match (range.get(0), range.get(1), range.get(2)) {
        (Some(f0), Some(f1), Some(f2)) => Value::IntRange(*f0, *f1, *f2),
        _ => Value::IntRange(0, 0, 0),
    };

    lexer.extras.parse_value(int_range);
    lexer.bump(lexer.remainder().len());
}

pub fn on_float_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);

    let float = if let Some(value) = lexer.remainder().trim().parse::<f32>().ok() {
        Value::Float(value)
    } else {
        Value::Float(0.0)
    };

    lexer.extras.parse_value(float);
    lexer.bump(lexer.remainder().len());
}

pub fn on_float_pair_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);

    let pair = from_comma_sep::<f32>(lexer);
    let float_pair = match (pair.get(0), pair.get(1)) {
        (Some(f0), Some(f1)) => Value::FloatPair(*f0, *f1),
        _ => Value::FloatPair(0.0, 0.0),
    };

    lexer.extras.parse_value(float_pair);
    lexer.bump(lexer.remainder().len());
}

pub fn on_float_range_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);

    let range = from_comma_sep::<f32>(lexer);

    let float_range = match (range.get(0), range.get(1), range.get(2)) {
        (Some(f0), Some(f1), Some(f2)) => Value::FloatRange(*f0, *f1, *f2),
        _ => Value::FloatRange(0.0, 0.0, 0.0),
    };

    lexer.extras.parse_value(float_range);
    lexer.bump(lexer.remainder().len());
}

pub fn on_binary_vec_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);

    let binary = match base64::decode(lexer.remainder().trim()) {
        Ok(content) => Value::BinaryVector(content),
        Err(_) => Value::BinaryVector(vec![]),
    };

    lexer.extras.parse_value(binary);
    lexer.bump(lexer.remainder().len());
}

pub fn on_symbol_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);

    let remaining = lexer.remainder().trim().to_string();

    let symbol_val = Value::Symbol(remaining);

    lexer.extras.parse_value(symbol_val);
    lexer.bump(lexer.remainder().len());
}

pub fn on_empty_attr(lexer: &mut Lexer<Attributes>) {    
    handle_comment(lexer);

    lexer.extras.parse_value(Value::Empty);
    lexer.bump(lexer.remainder().len());
}

pub fn on_complex_attr(lexer: &mut Lexer<Attributes>) {
    handle_comment(lexer);
    let idents = from_comma_sep::<String>(lexer);
    lexer
        .extras
        .parse_value(Value::Complex(BTreeSet::from_iter(idents)));
    lexer.bump(lexer.remainder().len());
}

pub fn from_comma_sep<T>(lexer: &mut Lexer<Attributes>) -> Vec<T>
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

pub fn handle_comment(lexer: &mut Lexer<Attributes>) {
    if lexer.remainder().trim_start().starts_with("<") {
        lexer.next();
    }
}

#[test]
#[tracing_test::traced_test]
fn test_attribute_parser() {
    // Test parsing add
    let parser = AttributeParser::default();
    let mut lexer = Attributes::lexer_with_extras("name .text <coments can only be immediately after the attribute type> cool_name", parser);
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

    // Test shortcut for defining an attribute without a name or value
    let mut parser = AttributeParser::default()
        .init(".shortcut cool shortcut");

    let shortcut = parser.next();
    assert_eq!(
        shortcut, 
        Some(atlier::system::Attribute::new(0, "shortcut", Value::Symbol("cool shortcut".to_string())))
    );

    // Test parsing .file attribute
    let mut parser = AttributeParser::default()
        .with_custom::<crate::parser::File>()
        .init("readme.md .file ./readme.md");

    let mut parsed = vec![];
    while let Some(attr) = parser.next() {
        parsed.push(attr);
    }
    eprintln!("{:#?}", parsed);

    // Test parsing .blob attribute
    let mut parser = AttributeParser::default()
        .with_custom::<crate::parser::BlobDescriptor>()
        .init("readme.md .blob sha256:testdigest");

    parser.define("readme", Value::Symbol("readme".to_string()));
    parser.define("extension", Value::Symbol("md".to_string()));

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
    assert_eq!(parser.next(), Some(atlier::system::Attribute::new(0, "custom", Value::Empty)));
    
    let mut parser = AttributeParser::default()
        .with_custom::<TestCustomAttr>()
        .init("custom <comment block> .custom-attr <comment block> test custom attr input");
    assert_eq!(parser.next(), Some(atlier::system::Attribute::new(0, "custom", Value::Empty)));
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


impl Into<Vec<Attribute>> for AttributeParser {
    fn into(mut self) -> Vec<Attribute> {
        self.parse_attribute();

        let mut attrs = vec![];

        if let Some(primary) = self.parsed {
            attrs.push(primary);
        } else {
            event!(Level::WARN, "Consuming parser without a primary attribute")
        }

        attrs.append(&mut self.properties);
        attrs
    }
}
