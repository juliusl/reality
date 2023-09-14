use std::collections::{BTreeSet, HashSet};
use std::ops::Deref;
use std::str::FromStr;
use std::{collections::HashMap, sync::Arc};

use crate::value::v2::ValueContainer;
use crate::{Value, Attribute, Block};
use logos::Lexer;
use logos::Logos;
use specs::shred::ResourceId;
use specs::{World, WorldExt, Entity, LazyUpdate};
use tracing::{event, trace, debug, error};
use tracing::Level;

use crate::SpecialAttribute;
use crate::parser::Elements;

use super::Attributes;
use super::CustomAttribute;

/// Parser for parsing attributes
///
#[derive(Default, Clone)]
pub struct AttributeParser<Attr = Attribute, Val = Value> {
    /// Resource Id of the output container,
    /// 
    resource_id: u64,
    /// Id of this attribute,
    /// 
    id: u32,
    /// Defaults to Value::Empty
    /// 
    value: Val,
    /// Attribute name, can also be referred to as tag
    /// 
    name: Option<String>,
    /// Transient symbol,
    /// 
    symbol: Option<String>,
    /// Transient value
    edit: Option<Val>,
    /// The parsed stable attribute
    parsed: Option<Attr>,
    /// Stack of transient attribute properties
    properties: Vec<Attr>,
    /// Custom attribute parsers
    attribute_table: HashMap<String, CustomAttribute>,
    /// Reference to world being edited
    storage: Option<Arc<World>>,
}

impl std::fmt::Debug for AttributeParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttributeParser")
            .field("id", &self.id)
            .field("value", &self.value)
            .field("name", &self.name)
            .field("symbol", &self.symbol)
            .field("edit", &self.edit)
            .field("parsed", &self.parsed)
            .field("properties", &self.properties)
            .field("attribute_table", &self.attribute_table)
            .field("storage", &self.storage.is_some())
            .finish()
    }
}

impl AttributeParser {
    pub fn init(&mut self, content: impl AsRef<str>) -> &mut Self {
        self.parse(content);
        self
    }

    /// Parses content, updating internal state
    ///
    pub fn parse(&mut self, content: impl AsRef<str>) -> &mut Self {
        let custom_attributes = self.attribute_table.clone();

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
                        Some(Elements::Identifier(custom_attr_type)) => {
                            let custom_attr_type = custom_attr_type.trim_start_matches(".").to_string();
                            let mut input = elements_lexer.remainder().trim().to_string();

                            let scanning = input.to_string();
                            let mut comment_lexer = Elements::lexer(&scanning);

                            while let Some(token) = &mut comment_lexer.next() {
                                match token {
                                    Elements::Comment(comment) => {
                                        input = input.replace(&format!("<{comment}>"), "");
                                    },
                                    _ => {}
                                }
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
                                    // This might be intended, but in case it is not
                                    // this event here is to help figure out config issues
                                    event!(
                                        Level::TRACE, 
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

    /// Sets the resource id for this parser,
    /// 
    pub fn set_resource_id(&mut self, resource_id: u64) {
        self.resource_id = resource_id;
    }

    /// Adds a custom attribute parser and returns self,
    ///
    pub fn with_custom<C>(&mut self) -> &mut Self 
    where
        C: SpecialAttribute
    {
        self.add_custom(CustomAttribute::new::<C>());
        self
    }

    /// Adds a custom attribute parser,
    ///
    pub fn add_custom(&mut self, custom_attr: impl Into<CustomAttribute>)
    {
        let custom_attr = custom_attr.into();
        self.attribute_table.insert(custom_attr.ident(), custom_attr);
    }

    /// Adds a custom attribute parser, 
    /// 
    /// Returns a clone of the custom attribute added,
    /// 
    pub fn add_custom_with(&mut self, ident: impl AsRef<str>, parse: fn(&mut AttributeParser, String)) -> CustomAttribute {
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
    pub fn set_edit(&mut self, value:  impl Into<Value>) {
        self.edit = Some(value.into());
    }

    /// Sets the current world being edited,
    /// 
    pub fn set_world(&mut self, world: Arc<World>) {
        self.storage = Some(world);
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

    /// Returns the current value,
    /// 
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Returns the last attribute on the stack, 
    /// 
    pub fn peek(&self) -> Option<&Attribute> {
        self.properties.last()
    }

    /// Returns an immutable reference to world,
    /// 
    pub fn world(&self) -> Option<&Arc<World>> {
        self.storage.as_ref()
    }

    /// Returns the entity that owns this parser,
    /// 
    pub fn entity(&self) -> Option<Entity> {
        self.world().and_then(|w| Some(w.entities().entity(self.id)))
    }

    /// Returns the last child entity created by this parser,
    /// 
    pub fn last_child_entity(&self) -> Option<Entity> {
        match self.peek().and_then(|p| Some(p.id())) {
            Some(ref child) if (*child != self.id)  => {
                self.world().and_then(|w| Some(w.entities().entity(*child)))
            },
            _ => None
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
    pub fn define_child(&mut self, entity: Entity, symbol: impl AsRef<str>, value: impl Into<Value>) {
        let id = self.id;
        self.set_id(entity.id());
        self.set_symbol(symbol);
        self.set_edit(value.into());
        self.set_value(Value::Empty);
        self.parse_attribute();
        self.set_id(id);
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

    /// Returns an iterator over special attributes installed on this parser,
    /// 
    pub fn iter_special_attributes(&self) -> impl Iterator<Item = (&String, &CustomAttribute)>{
        self.attribute_table.iter()
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
    let remaining = handle_comment(lexer);

    let text_buf = Value::TextBuffer(remaining);

    lexer.extras.parse_value(text_buf);

    lexer.bump(lexer.remainder().len());
}

pub fn on_bool_attr(lexer: &mut Lexer<Attributes>) {
    let bool_attr = if let Some(value) = handle_comment(lexer).parse().ok() {
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
    let int_attr = if let Some(value) = handle_comment(lexer).parse::<i32>().ok() {
        Value::Int(value)
    } else {
        Value::Int(0)
    };

    lexer.extras.parse_value(int_attr);
    lexer.bump(lexer.remainder().len());
}

pub fn on_int_pair_attr(lexer: &mut Lexer<Attributes>) {
    let pair = from_comma_sep::<i32>(lexer);

    let int_pair = match (pair.get(0), pair.get(1)) {
        (Some(f0), Some(f1)) => Value::IntPair(*f0, *f1),
        _ => Value::IntPair(0, 0),
    };

    lexer.extras.parse_value(int_pair);
    lexer.bump(lexer.remainder().len());
}

pub fn on_int_range_attr(lexer: &mut Lexer<Attributes>) {
    let range = from_comma_sep::<i32>(lexer);

    let int_range = match (range.get(0), range.get(1), range.get(2)) {
        (Some(f0), Some(f1), Some(f2)) => Value::IntRange(*f0, *f1, *f2),
        _ => Value::IntRange(0, 0, 0),
    };

    lexer.extras.parse_value(int_range);
    lexer.bump(lexer.remainder().len());
}

pub fn on_float_attr(lexer: &mut Lexer<Attributes>) {
    let float = if let Some(value) = handle_comment(lexer).parse::<f32>().ok() {
        Value::Float(value)
    } else {
        Value::Float(0.0)
    };

    lexer.extras.parse_value(float);
    lexer.bump(lexer.remainder().len());
}

pub fn on_float_pair_attr(lexer: &mut Lexer<Attributes>) {
    let pair = from_comma_sep::<f32>(lexer);
    let float_pair = match (pair.get(0), pair.get(1)) {
        (Some(f0), Some(f1)) => Value::FloatPair(*f0, *f1),
        _ => Value::FloatPair(0.0, 0.0),
    };

    lexer.extras.parse_value(float_pair);
    lexer.bump(lexer.remainder().len());
}

pub fn on_float_range_attr(lexer: &mut Lexer<Attributes>) {
    let range = from_comma_sep::<f32>(lexer);

    let float_range = match (range.get(0), range.get(1), range.get(2)) {
        (Some(f0), Some(f1), Some(f2)) => Value::FloatRange(*f0, *f1, *f2),
        _ => Value::FloatRange(0.0, 0.0, 0.0),
    };

    lexer.extras.parse_value(float_range);
    lexer.bump(lexer.remainder().len());
}

pub fn on_binary_vec_attr(lexer: &mut Lexer<Attributes>) {
    let binary = match base64::decode(handle_comment(lexer)) {
        Ok(content) => Value::BinaryVector(content),
        Err(_) => Value::BinaryVector(vec![]),
    };

    lexer.extras.parse_value(binary);
    lexer.bump(lexer.remainder().len());
}

pub fn on_symbol_attr(lexer: &mut Lexer<Attributes>) {
    let remaining = handle_comment(lexer);

    let symbol_val = Value::Symbol(remaining);

    lexer.extras.parse_value(symbol_val);
    lexer.bump(lexer.remainder().len());
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
    lexer.bump(lexer.remainder().len());
}

pub fn from_comma_sep<T>(lexer: &mut Lexer<Attributes>) -> Vec<T>
where
    T: FromStr,
{
    let input = handle_comment(lexer);
    input
        .split(",")
        .filter_map(|i| i.trim().parse().ok())
        .collect()
}

pub fn handle_comment(lexer: &mut Lexer<Attributes>) -> String {
    let mut input = lexer.remainder().trim().to_string();

    let scanning = input.to_string();
    let mut comment_lexer = Elements::lexer(&scanning);

    while let Some(token) = &mut comment_lexer.next() {
        match token {
            Elements::Comment(comment) => {
                input = input.replace(&format!("<{comment}>"), "");
            },
            _ => {}
        }
    }

    input.trim().to_string()
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
    let mut parser = AttributeParser::default();
    let parser = parser.init(".shortcut cool shortcut");

    let shortcut = parser.next();
    assert_eq!(
        shortcut, 
        Some(Attribute::new(0, "shortcut", Value::Symbol("cool shortcut".to_string())))
    );

    // Test parsing .blob attribute
    let mut parser = AttributeParser::default();
    let parser = parser
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

    let mut parser = AttributeParser::default();
    let parser = parser
        .with_custom::<TestCustomAttr>()
        .init("custom .custom-attr test custom attr input");
    assert_eq!(parser.next(), Some(Attribute::new(0, "custom", Value::Empty)));
    
    let mut parser = AttributeParser::default();
    let parser = parser
        .with_custom::<TestCustomAttr>()
        .init("custom <comment block> .custom-attr <comment block> test custom attr input <comment block>");
    assert_eq!(parser.next(), Some(Attribute::new(0, "custom", Value::Empty)));

    let mut parser = AttributeParser::default();
    let parser = parser.with_custom::<TestCustomAttr>()
        .init("custom <comment block> .symbol <comment block> test custom attr input <comment block>");
    assert_eq!(parser.next(), Some(Attribute::new(0, "custom", Value::Symbol("test custom attr input".to_string()))));

    
    let mut parser = AttributeParser::default();
    let parser = parser.with_custom::<TestCustomAttr>()
        .init("custom <comment block> .int_pair <comment block 1> 1, <comment block2>5 <comment block>");

    assert_eq!(parser.next(), Some(Attribute::new(0, "custom", Value::IntPair(1, 5))));

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

use runmd::prelude::*;

/// Resource for storing custom attribute parse functions
/// 
#[derive(Clone)]
pub struct CustomAttributeContainer(HashSet<CustomAttribute>);

#[runmd::prelude::async_trait]
impl ExtensionLoader for super::AttributeParser {
    async fn load_extension(&self, extension: &str, input: Option<&str>) -> Option<BoxedNode> {
        let mut parser = self.clone();

        // Forwards-compatibility support
        const V1_ATTRIBUTES_EXTENSION: &'static str = "application/repo.reality.attributes.v1";
        if extension == V1_ATTRIBUTES_EXTENSION {
            debug!("Enabling v1 attribute parsers");
            parser.with_custom::<ValueContainer<bool>>()
                .with_custom::<ValueContainer<i32>>()
                .with_custom::<ValueContainer<[i32; 2]>>()
                .with_custom::<ValueContainer<[i32; 3]>>()
                .with_custom::<ValueContainer<f32>>()
                .with_custom::<ValueContainer<[f32; 2]>>()
                .with_custom::<ValueContainer<[f32; 3]>>()
                .with_custom::<ValueContainer<String>>()
                .with_custom::<ValueContainer<&'static str>>()
                .with_custom::<ValueContainer<Vec<u8>>>()
                .with_custom::<ValueContainer<BTreeSet<String>>>();
            parser.add_custom_with("true", |parser, _| parser.set_value(true));
            parser.add_custom_with("false", |parser, _| parser.set_value(false));
            parser.add_custom_with("int_pair", ValueContainer::<[i32; 2]>::parse);
            parser.add_custom_with("int_range", ValueContainer::<[i32; 3]>::parse);
            parser.add_custom_with("float_pair", ValueContainer::<[f32; 2]>::parse);
            parser.add_custom_with("float_range", ValueContainer::<[f32; 3]>::parse);
        }

        // V2 "Plugin" model
        // application/repo.reality.attributes.v1.custom.<plugin-name>;
        /*
            <application/repo.reality.attributes.v1.custom>
            <..request>
        */
        if extension.starts_with("application/repo.reality.attributes.v1.custom") {
            if let Some((prefix, plugin)) = extension.rsplit_once('.') {
                if prefix == "application/repo.reality.attributes.v1" && plugin == "custom" { 
                    if let Some(plugins) = parser.storage.clone().and_then(|world| world.try_fetch::<CustomAttributeContainer>().map(|c| c.deref().clone())) {
                        debug!("Loading plugins");
                        for plugin in plugins.0.iter() {
                            parser.add_custom(plugin.clone());
                        }

                        // Increment the id since this is a new extension
                        parser.id += 1;
                    }
                } else if prefix == "application/repo.reality.attributes.v1.custom" {
                    if let Some(plugin) = self.attribute_table.get(plugin) {
                        plugin.parse(&mut parser, input.unwrap_or_default());
                    }
                } else {

                }
            }
        }

        Some(Box::pin(parser))
    }
}

impl Node for super::AttributeParser {
    fn set_info(&mut self, node_info: NodeInfo, _block_info: BlockInfo) {
        if self.id == 0 {
            if let Some(parent) = node_info.parent_idx.as_ref() {
                self.set_id(*parent as u32);
            } else {
                self.set_id(node_info.idx as u32);
            }
        } else {
            // Only set if this node originates from loading a custom plugin extension
        }
    }

    fn define_property(&mut self, name: &str, tag: Option<&str>, input: Option<&str>) {
        if let Some(tag) = tag.as_ref() {
            self.set_name(tag);
            self.set_symbol(name);
        } else {
            self.set_name(name);
        }

        match self.attribute_table.get(name).cloned() {
            Some(cattr) => {
                cattr.parse(self, input.unwrap_or_default());
                self.parse_attribute();
            },
            None => {
                trace!(attr_ty=name, "Did not have attribute");
            },
        }
    }

    fn completed(mut self: Box<Self>) {
        let mut attrs = vec![];
        while let Some(next) = self.next() {
            attrs.push(next);
        }

        let resource_id = ResourceId::new_with_dynamic_id::<Block>(self.resource_id);
        self.lazy_exec_mut(move |_world| {
            if let Some(mut block) = _world.try_fetch_mut_by_id::<Block>(resource_id) {
                for attr in attrs.iter(){
                    block.add_attribute(attr);
                }
            } else {
                error!("Could not retrieve block");
            }
        });

        // TODO -- Make actual storage dependency swappable here.
    }
}

#[tokio::test]
async fn test_v2_parser() {
    let parser = AttributeParser::default();

    let mut parser = parser.load_extension("application/repo.reality.attributes.v1", None).await.expect("should return a node");
    parser.define_property("int", Some("test"), Some("256"));
    parser.define_property("float", Some("test-2"), Some("256.0"));

    println!("{:#?}", parser);
}