use std::collections::BTreeSet;
use std::ops::Range;
use std::str::FromStr;
use std::{collections::HashMap, sync::Arc};

use crate::{Attribute, Keywords, Value, Identifier};
use logos::Logos;
use logos::{Lexer, Span};
use specs::{Entity, LazyUpdate, World, WorldExt};
use tracing::Level;
use tracing::{event, trace};

use crate::parser::Elements;
use crate::SpecialAttribute;

use super::CustomAttribute;
use super::{Attributes, PropertyAttribute};

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
    /// Transient property symbol,
    symbol: Option<String>,
    /// Transient value
    edit: Option<Value>,
    /// Attribute type identifier,
    attr_ident: Option<String>,
    /// The parsed stable attribute
    parsed: Option<Attribute>,
    /// Stack of transient attribute properties
    properties: Vec<Attribute>,
    /// Custom attribute parsers
    custom_attributes: HashMap<String, CustomAttribute>,
    /// Reference to world being edited
    world: Option<Arc<World>>,
    /// Last char count,
    last_parsed_char_count: usize,
    /// Default custom attribute,
    ///
    default_custom_attribute: Option<CustomAttribute>,
    /// Default property attribute,
    ///
    default_property_attribute: Option<PropertyAttribute>,
    /// Keyword that preceeded this parser,
    ///
    keyword: Option<Keywords>,
    /// Current block identifier,
    /// 
    block_identifier: Identifier,
    /// Implicit identifier to use as the base identifier,
    /// 
    implicit_identifier: Option<Identifier>,
    /// The current identifier being built,
    /// 
    current_identifier: Identifier,
    /// Line count
    ///
    line_count: usize,
    /// Whenever parse identifier is called this is updated,
    /// 
    parsed_idents: Vec<String>,
}

impl AttributeParser {
    /// Sets the default custom attribute,
    ///
    /// If no custom attribute is found this will be called,
    ///
    pub fn set_default_custom_attribute(&mut self, default: CustomAttribute) {
        self.default_custom_attribute = Some(default);
    }

    /// Sets the default property attribute,
    ///
    /// If a property attribute is found this will be called if set,
    ///
    pub fn set_default_property_attribute(&mut self, default: PropertyAttribute) {
        self.default_property_attribute = Some(default);
    }

    /// Sets the current block identifierm
    /// 
    pub fn set_block_identifier(&mut self, ident: &Identifier) {
        self.block_identifier = ident.clone();
    }

    /// Sets the implicit identifier, chainable
    /// 
    pub fn with_implicit_identifier(&mut self, ident: Option<&Identifier>) -> &mut Self {
        if let Some(replaced) = self.implicit_identifier.take() {
            trace!("Replacing implicit identifier {:#}", replaced);
        }

        self.implicit_identifier = ident.cloned();
        self
    }

    /// Returns the current identifier,
    /// 
    pub fn current_identifier(&self) -> &Identifier {
        &self.current_identifier
    }

    /// Returns the current block identifier,
    /// 
    pub fn block_identifier(&self) -> &Identifier {
        &self.block_identifier
    }

    /// Sets the current keyword, chainbable,
    ///
    pub fn with_keyword(&mut self, keyword: Keywords) -> &mut Self {
        self.keyword = Some(keyword);
        self
    }

    /// Initializes self w/ content,
    /// 
    pub fn init(&mut self, content: impl AsRef<str>) -> &mut Self {
        self.parse(content);
        self
    }

    /// Parses content, updating internal state
    ///
    pub fn parse(&mut self, content: impl AsRef<str>) -> &mut Self {
        self.line_count += 1;

        self.parsed_idents.clear();

        // Reset the current identifier
        if let Some(implicit) = self.implicit_identifier.as_ref() {
            self.current_identifier = implicit.clone();
        } else {
            self.current_identifier = Identifier::default();
        }

        let mut lexer = Attributes::lexer_with_extras(content.as_ref(), self.clone());

        let mut parsed_len = 0;
        while let Some(token) = lexer.next() {
            match token {
                Attributes::Error
                    if lexer.slice().is_empty()
                        || lexer.slice() == ":"
                        || lexer.slice() == "`"
                        || lexer.slice() == "+" =>
                {
                    trace!("Error {}", lexer.slice());
                    break;
                }
                _ => {
                    parsed_len += lexer.slice().len();
                    event!(
                        Level::TRACE,
                        "Parsed {:?} {}, {}",
                        token,
                        lexer.slice(),
                        lexer.slice().len()
                    );

                    match token {
                        Attributes::Empty
                        | Attributes::Bool
                        | Attributes::Int
                        | Attributes::IntPair
                        | Attributes::IntRange
                        | Attributes::Float
                        | Attributes::FloatPair
                        | Attributes::FloatRange
                        | Attributes::Symbol
                        | Attributes::Complex
                        | Attributes::Text
                        | Attributes::BinaryVector
                            if self.default_property_attribute.is_some() =>
                        {
                            let default_property = self
                                .default_property_attribute
                                .as_ref()
                                .expect("should exist, just checked");
                            default_property.on_property_attribute(&mut lexer.extras);
                        }
                        _ => {}
                    }

                    match token {
                        Attributes::Empty
                        | Attributes::Bool
                        | Attributes::Int
                        | Attributes::IntPair
                        | Attributes::IntRange
                        | Attributes::Float
                        | Attributes::FloatPair
                        | Attributes::FloatRange
                        | Attributes::Symbol
                        | Attributes::Complex
                        | Attributes::Text
                        | Attributes::BinaryVector
                        | Attributes::NewLine
                        | Attributes::Custom => break,
                        _ => {}
                    }
                }
            }
        }

        *self = lexer.extras;
        self.last_parsed_char_count = parsed_len;
        self.parse_attribute();
        self
    }

    /// Adds a custom attribute parser and returns self,
    ///
    pub fn with_custom<C>(&mut self) -> &mut Self
    where
        C: SpecialAttribute,
    {
        self.add_custom(CustomAttribute::new::<C>());
        self
    }

    /// Adds a custom attribute parser,
    ///
    pub fn add_custom(&mut self, custom_attr: impl Into<CustomAttribute>) {
        let custom_attr = custom_attr.into();
        trace!("Adding custom parser {}", custom_attr.ident());
        self.custom_attributes
            .insert(custom_attr.ident(), custom_attr);
    }

    /// Adds a custom attribute parser,
    ///
    /// Returns a clone of the custom attribute added,
    ///
    pub fn add_custom_with(
        &mut self,
        ident: impl AsRef<str>,
        parse: fn(&mut AttributeParser, String),
    ) -> CustomAttribute {
        let attr = CustomAttribute::new_with(ident, parse);
        self.add_custom(attr.clone());
        attr
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

    /// Returns the property symbol,
    ///
    pub fn property(&self) -> Option<&String> {
        self.symbol.as_ref()
    }

    /// Returns the current value,
    ///
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Returns the current edit value,
    ///
    pub fn edit_value(&self) -> Option<&Value> {
        self.edit.as_ref()
    }

    /// Returns the current attr ident,
    ///
    pub fn attr_ident(&self) -> Option<&String> {
        self.attr_ident.as_ref()
    }

    /// Returns the current keyword,
    ///
    pub fn keyword(&self) -> Option<&Keywords> {
        self.keyword.as_ref()
    }

    /// Returns the current line count,
    ///
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// Returns the last attribute on the stack,
    ///
    pub fn peek(&self) -> Option<&Attribute> {
        self.properties.last()
    }

    /// Returns an immutable reference to world,
    ///
    pub fn world(&self) -> Option<&Arc<World>> {
        self.world.as_ref()
    }

    /// Returns the entity that owns this parser,
    ///
    pub fn entity(&self) -> Option<Entity> {
        self.world()
            .and_then(|w| Some(w.entities().entity(self.id)))
    }

    /// Returns the last child entity created by this parser,
    ///
    pub fn last_child_entity(&self) -> Option<Entity> {
        match self.peek().and_then(|p| Some(p.id())) {
            Some(ref child) if (*child != self.id) => {
                self.world().and_then(|w| Some(w.entities().entity(*child)))
            }
            _ => None,
        }
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

    /// Pushes an attribute on to the property stack,
    ///
    /// This method can be used directly when configuring a child entity.
    ///
    pub fn define_child(
        &mut self,
        entity: Entity,
        symbol: impl AsRef<str>,
        value: impl Into<Value>,
    ) {
        let id = self.id;
        self.set_id(entity.id());
        self.set_symbol(symbol);
        self.set_edit(value.into());
        self.set_value(Value::Empty);
        self.parse_attribute();
        self.set_id(id);
    }

    pub fn try_define_child(&mut self, symbol: impl AsRef<str>, value: impl Into<Value>) {
        if let Some(last_entity) = self.last_child_entity() {
            self.define_child(last_entity, symbol, value);
        }
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
                let name = self
                    .attr_ident
                    .as_ref()
                    .filter(|_| self.keyword().is_some())
                    .map(|suffix| {
                        if suffix != &name {
                            format!("{name}.{suffix}.{}", value.symbol().unwrap_or_default())
                        } else {
                            format!("{name}.{}", value.symbol().unwrap_or_default())
                        }
                    })
                    .unwrap_or(name);

                // let symbol = self
                //     .extension_namespace()
                //     .map(|p| format!("{p}.{symbol}"))
                //     .unwrap_or(symbol);

                let mut attr = Attribute::new(self.id, format!("{name}::{symbol}"), value);
                attr.edit_as(edit);
                self.properties.push(attr);
            }
            (Some(name), None, value, None) => {
                let name = self
                    .attr_ident
                    .as_ref()
                    .filter(|_| self.keyword().is_some())
                    .map(|suffix| {
                        if suffix != &name {
                            format!("{name}.{suffix}.{}", value.symbol().unwrap_or_default())
                        } else {
                            format!("{name}.{}", value.symbol().unwrap_or_default())
                        }
                    })
                    .unwrap_or(name);

                let attr = Attribute::new(self.id, name, value);
                if self.parsed.is_some() && self.keyword != Some(Keywords::Define) {
                    event!(Level::DEBUG, "Replacing parsed attribute");
                    self.parsed = Some(attr);
                } else if self.parsed.is_none() {
                    self.parsed = Some(attr);
                }
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

    /// Parses an identifier,
    ///
    /// In this context, this is either a name or symbol.
    ///
    fn parse_identifier(&mut self, symbol: String) {
        self.parsed_idents.push(symbol.clone());

        if self.name.is_none() {
            self.set_name(symbol)
        } else {
            self.set_symbol(symbol)
        }
    }

    /// Returns an iterator over special attributes installed on this parser,
    ///
    pub fn iter_special_attributes(&self) -> impl Iterator<Item = (&String, &CustomAttribute)> {
        self.custom_attributes.iter()
    }

    /// Lazily executes exec mut fn w/ world if the world is set,
    ///
    pub fn lazy_exec_mut(&mut self, exec: impl FnOnce(&mut World) + 'static + Send + Sync) {
        if let Some(world) = self.world() {
            let lazy_update = world.read_resource::<LazyUpdate>();

            lazy_update.exec_mut(exec);
        }
    }

    /// Lazily execute exec fn w/ world if the world is set,
    ///
    pub fn lazy_exec(&mut self, exec: impl FnOnce(&World) + 'static + Send + Sync) {
        if let Some(world) = self.world() {
            let lazy_update = world.read_resource::<LazyUpdate>();

            // It looks like the fn signatures are the same, so this enforces immutable access,
            lazy_update.exec(|world| exec(&world));
        }
    }

    /// Returns the length of the last parse,
    ///
    pub fn last_parse_len(&self) -> usize {
        self.last_parsed_char_count
    }
}

pub fn on_identifier(lexer: &mut Lexer<Attributes>) {
    let slice = lexer.slice();
    lexer.extras.parse_identifier(slice.to_string());
}

pub fn on_comment_start(lexer: &mut Lexer<Attributes>) {
    let end_pos = lexer
        .remainder()
        .lines()
        .take(1)
        .next()
        .and_then(|s| s.find(">"))
        .expect("Didn't find a closing `>`");

    lexer.bump(end_pos + 1);
}

pub fn on_text_attr(lexer: &mut Lexer<Attributes>) {
    let remaining = handle_input_extraction(lexer);

    let text_buf = Value::TextBuffer(remaining.value());

    lexer.extras.parse_value(text_buf);
}

pub fn on_bool_attr(lexer: &mut Lexer<Attributes>) {
    let bool_attr = if let Some(value) = handle_input_extraction(lexer).value().parse().ok() {
        Value::Bool(value)
    } else {
        Value::Bool(false)
    };

    lexer.extras.parse_value(bool_attr);
}

pub fn on_bool_enable(lexer: &mut Lexer<Attributes>) {
    lexer.extras.parse_value(Value::Bool(true));
}

pub fn on_bool_disable(lexer: &mut Lexer<Attributes>) {
    lexer.extras.parse_value(Value::Bool(false));
}

pub fn on_int_attr(lexer: &mut Lexer<Attributes>) {
    let int_attr = if let Some(value) = handle_input_extraction(lexer).value().parse::<i32>().ok() {
        Value::Int(value)
    } else {
        Value::Int(0)
    };

    lexer.extras.parse_value(int_attr);
}

pub fn on_int_pair_attr(lexer: &mut Lexer<Attributes>) {
    let pair = from_comma_sep::<i32>(lexer);

    let int_pair = match (pair.get(0), pair.get(1)) {
        (Some(f0), Some(f1)) => Value::IntPair(*f0, *f1),
        _ => Value::IntPair(0, 0),
    };

    lexer.extras.parse_value(int_pair);
}

pub fn on_int_range_attr(lexer: &mut Lexer<Attributes>) {
    let range = from_comma_sep::<i32>(lexer);

    let int_range = match (range.get(0), range.get(1), range.get(2)) {
        (Some(f0), Some(f1), Some(f2)) => Value::IntRange(*f0, *f1, *f2),
        _ => Value::IntRange(0, 0, 0),
    };

    lexer.extras.parse_value(int_range);
}

pub fn on_float_attr(lexer: &mut Lexer<Attributes>) {
    let float = if let Some(value) = handle_input_extraction(lexer).value().parse::<f32>().ok() {
        Value::Float(value)
    } else {
        Value::Float(0.0)
    };

    lexer.extras.parse_value(float);
}

pub fn on_float_pair_attr(lexer: &mut Lexer<Attributes>) {
    let pair = from_comma_sep::<f32>(lexer);
    let float_pair = match (pair.get(0), pair.get(1)) {
        (Some(f0), Some(f1)) => Value::FloatPair(*f0, *f1),
        _ => Value::FloatPair(0.0, 0.0),
    };

    lexer.extras.parse_value(float_pair);
}

pub fn on_float_range_attr(lexer: &mut Lexer<Attributes>) {
    let range = from_comma_sep::<f32>(lexer);

    let float_range = match (range.get(0), range.get(1), range.get(2)) {
        (Some(f0), Some(f1), Some(f2)) => Value::FloatRange(*f0, *f1, *f2),
        _ => Value::FloatRange(0.0, 0.0, 0.0),
    };

    lexer.extras.parse_value(float_range);
}

pub fn on_binary_vec_attr(lexer: &mut Lexer<Attributes>) {
    let binary = match base64::decode(handle_input_extraction(lexer).value()) {
        Ok(content) => Value::BinaryVector(content),
        Err(_) => Value::BinaryVector(vec![]),
    };

    lexer.extras.parse_value(binary);
}

pub fn on_symbol_attr(lexer: &mut Lexer<Attributes>) {
    let remaining = handle_input_extraction(lexer);

    let symbol_val = Value::Symbol(remaining.value());

    lexer.extras.parse_value(symbol_val);
}

pub fn on_empty_attr(lexer: &mut Lexer<Attributes>) {
    lexer.extras.parse_value(Value::Empty);
    lexer.bump(lexer.remainder().len());
}

pub fn on_complex_attr(lexer: &mut Lexer<Attributes>) {
    let idents = from_comma_sep::<String>(lexer);
    lexer
        .extras
        .parse_value(Value::Complex(BTreeSet::from_iter(idents)));
}

pub fn on_custom_attr(lexer: &mut Lexer<Attributes>) {
    let custom_attr_type = &lexer.slice()[1..].to_string();

    let input = handle_input_extraction(lexer);

    if lexer.extras.name().is_none() {
        lexer.extras.set_name(custom_attr_type.to_string());
    }

    lexer.extras.set_value(Value::Symbol(
        input
            .value()
            .trim_start_matches(custom_attr_type)
            .trim()
            .to_string(),
    ));

    lexer.extras.set_edit(Value::Symbol(input.value()));

    match lexer.extras.keyword {
        Some(Keywords::Add) => {
            lexer.extras.current_identifier.join(custom_attr_type).ok();

            for tag in lexer.extras.parsed_idents.drain(..) {
                lexer.extras.current_identifier.add_tag(tag);
            }
        }
        Some(Keywords::Extension) => {
            if let Some(implicit_identifier) = lexer.extras.implicit_identifier.as_mut() {
                implicit_identifier.join(custom_attr_type).ok();
            } else {
                lexer.extras.with_implicit_identifier(Identifier::default().join(custom_attr_type).ok().as_deref());
            }

            if !input.value().is_empty() {
                lexer.extras.implicit_identifier.as_mut().map(|i| i.join(input.value()).ok());
            }

            for tag in lexer.extras.parsed_idents.drain(..) {
                trace!("setting tag {tag}");
                lexer.extras.implicit_identifier.as_mut().map(|i| i.add_tag(&tag));
                lexer.extras.current_identifier.add_tag(tag);
            }

            lexer.extras.current_identifier.join(custom_attr_type).ok();
        }
        _ => {
        }
    }

    lexer.extras.attr_ident = Some(custom_attr_type.to_string());

    trace!("Checking for custom attribute");
    let custom_parser = lexer
        .extras
        .custom_attributes
        .get(custom_attr_type)
        .cloned();
    match custom_parser {
        Some(custom_attr) => {
            custom_attr.parse(&mut lexer.extras, input.value());
        }
        None if lexer.extras.default_custom_attribute.is_some() => {
            let custom_attr = lexer
                .extras
                .default_custom_attribute
                .clone()
                .expect("should exist, just checked");

            custom_attr.parse(&mut lexer.extras, input.value());
            match lexer.extras.keyword {
                Some(Keywords::Extension) => {
                    // lexer.extras.extension_namespace = lexer.extras.attr_ident.clone();
                }
                _ => {}
            }
        }
        None => {
            // This might be intended, but in case it is not
            // this event here is to help figure out config issues
            event!(
                Level::TRACE,
                "Did not parse {custom_attr_type}, could not find custom attribute parser",
            );
        }
    }

    lexer.extras.attr_ident.take();
}

pub fn from_comma_sep<T>(lexer: &mut Lexer<Attributes>) -> Vec<T>
where
    T: FromStr,
{
    let input = handle_input_extraction(lexer);
    input
        .value()
        .split(",")
        .filter_map(|i| i.trim().parse().ok())
        .collect()
}

fn handle_input_extraction(lexer: &mut Lexer<Attributes>) -> ParserInputInfo {
    let mut input = lexer.remainder().to_string();

    let scanning = input.to_string();
    let mut comment_lexer = Elements::lexer(&scanning);

    let mut parser_input_info = ParserInputInfo::new(input.to_string());
    while let Some(token) = &mut comment_lexer.next() {
        match token {
            Elements::NewLine | Elements::InlineOperator | Elements::Error => {
                parser_input_info.set_end_pos(comment_lexer.span().start);
                lexer.bump(parser_input_info.end_pos);

                return parser_input_info;
            }
            Elements::Comment(comment) => {
                input = input.replace(&format!("<{comment}>"), "");
                parser_input_info.add_comment(comment_lexer.span());
            }
            _ => {}
        }
    }

    lexer.bump(parser_input_info.end_pos);
    parser_input_info
}

#[derive(Default, Clone)]
struct ParserInputInfo {
    original: String,
    comments: Vec<Range<usize>>,
    end_pos: usize,
}

impl ParserInputInfo {
    fn new(original: String) -> Self {
        let end_pos = original.len();
        ParserInputInfo {
            original,
            end_pos,
            ..Default::default()
        }
    }

    fn add_comment(&mut self, span: Span) {
        let start = if span.start > 0 { span.start - 1 } else { 0 };

        let end = if span.end > 0 { span.end - 1 } else { 0 };

        self.comments.push(start..end);
    }

    fn set_end_pos(&mut self, pos: usize) {
        self.end_pos = pos;
    }

    fn value(&self) -> String {
        if !self.comments.is_empty() {
            use std::fmt::Write;

            let mut sb = String::new();

            let mut start_pos = 0;
            for comment in self.comments.iter() {
                if let Ok(()) = write!(
                    sb,
                    "{}",
                    &self.original[..self.end_pos][start_pos..comment.start]
                ) {
                    start_pos = comment.end + 1;
                }
            }

            write!(sb, "{}", &self.original[..self.end_pos][start_pos..]).ok();

            sb.trim().to_string()
        } else {
            self.original[..self.end_pos].trim().to_string()
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::{Attribute, AttributeParser, Attributes, SpecialAttribute, Value, Keywords};
    use logos::Logos;

    #[test]
    #[tracing_test::traced_test]
    fn test_attribute_parser() {
        // Test parsing add
        let parser = AttributeParser::default();
        let mut lexer = Attributes::lexer_with_extras(
            "name .text /* comments can only be immediately after the attribute type */ cool_name",
            parser,
        );
        assert_eq!(lexer.next(), Some(Attributes::Identifier));
        assert_eq!(lexer.next(), Some(Attributes::Whitespace));
        assert_eq!(lexer.next(), Some(Attributes::Text));
        lexer.extras.parse_attribute();

        let attr = lexer.extras.next().expect("parses");
        assert_eq!(attr.name, "name");
        assert_eq!(attr.value, Value::TextBuffer("cool_name".to_string()));

        // Test parsing define
        let parser = AttributeParser::default();
        let mut lexer = Attributes::lexer_with_extras("connection name .text cool_name", parser);
        assert_eq!(lexer.next(), Some(Attributes::Identifier));
        assert_eq!(lexer.next(), Some(Attributes::Whitespace));
        assert_eq!(lexer.next(), Some(Attributes::Identifier));
        assert_eq!(lexer.next(), Some(Attributes::Whitespace));
        assert_eq!(lexer.next(), Some(Attributes::Text));
        lexer.extras.parse_attribute();

        let attr = lexer.extras.next().expect("parses");
        assert_eq!(attr.name, "connection::name");
        assert_eq!(attr.value, Value::Empty);
        assert_eq!(
            Value::TextBuffer("cool_name".to_string()),
            attr.transient.unwrap().1
        );
    }

    struct TestCustomAttr();

    impl SpecialAttribute for TestCustomAttr {
        fn ident() -> &'static str {
            "custom-attr"
        }

        fn parse(parser: &mut AttributeParser, _: impl AsRef<str>) {
            parser.set_value(Value::Empty);
        }
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

impl AsMut<AttributeParser> for AttributeParser {
    fn as_mut(&mut self) -> &mut AttributeParser {
        self
    }
}
