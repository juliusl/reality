use std::str::FromStr;

use atlier::system::{Attribute, Value};
use logos::{Lexer, Logos};
use tracing::{event, Level};

/// Parser for parsing attributes 
/// 
#[derive(Default)]
pub struct AttributeParser {
    id: u32,
    name: Option<String>,
    symbol: Option<String>,
    value: Value,
    edit: Option<Value>,
}

impl AttributeParser {
    /// Parses content, updating internal state
    /// 
    pub fn parse(self, content: impl AsRef<str>) -> Self {
        let mut lexer = Attributes::lexer_with_extras(content.as_ref(), self);
        
        while let Some(token) = lexer.next() {
            event!(Level::TRACE, "parsed {:?}", token);
        }

        lexer.extras
    }

    /// Parses the current state of the parser
    ///
    pub fn add(&mut self) -> Option<Attribute> {
        let name = self.name.take();
        let _symbol = self.symbol.take();
        let value = self.value.clone();
        let _edit = self.edit.take();

        match (name, value) {
            (Some(name), value) => Some(Attribute::new(self.id, name, value)),
            _ => None,
        }
    }

    /// Parses the current state of the parser
    ///
    pub fn define(&mut self) -> Option<Attribute> {
        let name = self.name.take();
        let symbol = self.symbol.take();
        let value = self.value.clone();
        let edit = self.edit.take();

        match (name, symbol, value, edit) {
            (Some(name), Some(symbol), value, Some(edit)) => {
                let mut attr = Attribute::new(self.id, format!("{name}::{symbol}"), value);
                attr.edit_as(edit);
                Some(attr)
            }
            _ => None,
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
    pub fn set_value(&mut self, value: Value) {
        self.value = value;
    }

    /// Sets the current transient value for the parser 
    /// 
    pub fn set_edit(&mut self, value: Value) {
        self.edit = Some(value);
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
        }
    }
}

#[derive(Logos, Debug, PartialEq, Eq)]
#[logos(extras = AttributeParser)]
enum Attributes {
    /// Symbol text, this is either name or symbol name
    ///
    #[regex("[A-Za-z]+[A-Za-z-._:0-9]*", on_symbol)]
    Symbol,
    /// text element parses all remaining text after .TEXT as a string
    #[token(".text", on_text_attr)]
    Text,
    /// bool element parses remaining as bool
    #[token(".enable", on_bool_enable)]
    #[token(".disable", on_bool_disable)]
    #[token(".bool", on_bool_attr)]
    Bool,
    /// int element parses remaining as i32
    #[token(".int", on_int_attr)]
    Int,
    /// int pair element parses remaining as 2 comma-delimmited i32's
    #[token(".int_pair", on_int_pair_attr)]
    IntPair,
    /// int range element parses remaining as 3 comma-delimitted i32's
    #[token(".int_range", on_int_range_attr)]
    IntRange,
    /// float element parses remaining as f32
    #[token(".float", on_float_attr)]
    Float,
    /// float pair element parses reamining as 2 comma delimitted f32's
    #[token(".float_pair", on_float_pair_attr)]
    FloatPair,
    /// float range element parses remaining as 3 comma delimitted f32's
    #[token(".float_range", on_float_range_attr)]
    FloatRange,
    /// binary vector element, currently parses the remaining as base64 encoded data
    #[token(".bin", on_binary_vec_attr)]
    #[token(".base64", on_binary_vec_attr)]
    BinaryVector,
    /// symbol value implies that the value is of symbolic quality,
    /// and though no explicit validations are in place, the value of the symbol
    /// should be valid in many contexts that require an identifier
    #[token(".symbol", on_symbol_attr)]
    SymbolValue,
    /// empty element parses
    #[token(".empty", on_empty_attr)]
    Empty,
    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

fn on_symbol(lexer: &mut Lexer<Attributes>) {
    let mut slice = lexer.slice();
    if slice.starts_with('#') {
        slice = &slice[1..];
    }

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
        (Some(f0), Some(f1)) =>  Value::FloatPair(*f0, *f1),
        _ => {
            Value::FloatPair(0.0, 0.0)
        },
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
fn test_attribute_parser() {
    // Test parsing add 
    let parser = AttributeParser::default();
    let mut lexer = Attributes::lexer_with_extras("name .text cool_name", parser);
    assert_eq!(lexer.next(), Some(Attributes::Symbol));
    assert_eq!(lexer.next(), Some(Attributes::Text));

    let attr = lexer.extras.add().expect("parses");
    assert_eq!(attr.name, "name");
    assert_eq!(attr.value, Value::TextBuffer("cool_name".to_string()));

    // Test parsing define 
    let parser = AttributeParser::default();
    let mut lexer = Attributes::lexer_with_extras("connection name .text cool_name", parser);
    assert_eq!(lexer.next(), Some(Attributes::Symbol));
    assert_eq!(lexer.next(), Some(Attributes::Symbol));
    assert_eq!(lexer.next(), Some(Attributes::Text));

    let attr = lexer.extras.define().expect("parses");
    assert_eq!(attr.name, "connection::name");
    assert_eq!(attr.value, Value::Empty);
    assert_eq!(attr.transient.unwrap().1, Value::TextBuffer("cool_name".to_string()));
}