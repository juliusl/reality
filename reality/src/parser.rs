use logos::Logos;
use specs::Entity;
use specs::World;
use specs::WorldExt;
use tracing::trace;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::event;
use tracing::Level;

use crate::Metadata;
use crate::Block;
use crate::BlockIndex;
use crate::BlockProperties;

mod attributes;
pub use attributes::AttributeParser;
pub use attributes::Attributes;
pub use attributes::BlobDescriptor;
pub use attributes::CustomAttribute;
pub use attributes::PropertyAttribute;
pub use attributes::SpecialAttribute;

mod keywords;
pub use keywords::Keywords;

mod elements;
pub use elements::Elements;

/// Parser for .runmd
///
/// When parsing configures a specs entity for each block parsed.
///
pub struct Parser {
    /// World storage
    world: Arc<World>,
    /// Root block
    root: Block,
    /// Reverse lookup entity from block name/symbol
    index: HashMap<String, Entity>,
    /// Named blocks
    blocks: BTreeMap<Entity, Block>,
    /// The current block being parsed
    parsing: Option<Entity>,
    /// Vector of custom attribute parsers to add to the default,
    /// attribute parser
    custom_attributes: Vec<CustomAttribute>,
    /// Stack of attribute parsers,
    parser_stack: Vec<AttributeParser>,
    /// Setting this field will interpret the root block implicitly as a control block, and control blocks as event blocks,
    /// Using the value of this field as the symbol.
    ///
    implicit_block_symbol: Option<String>,
    /// Default custom attribute,
    ///
    /// If set, will be included with each new attribute parser
    ///
    default_custom_attribute: Option<CustomAttribute>,
    /// Default property attribute,
    ///
    /// If set, will be included with each new attribute parser
    ///
    default_property_attribute: Option<PropertyAttribute>,
    /// True when the lexer is within block delimitter boundaries,
    ///
    /// If false, keywords will skip
    ///
    enabled: bool,
    /// Metadata including runmd source and current cursor position,
    /// 
    pub metadata: Metadata,
}

impl AsRef<World> for Parser {
    fn as_ref(&self) -> &World {
        &self.world
    }
}

impl Into<World> for Parser {
    fn into(self) -> World {
        match Arc::try_unwrap(self.world) {
            Ok(mut world) => {
                let mut fixed_hash_map = HashMap::<String, Entity>::default();
                for (key, value) in self.index.iter() {
                    fixed_hash_map.insert(key.trim().to_string(), value.clone());
                }
                world.insert(fixed_hash_map);
                world.insert(self.custom_attributes);

                world
                    .write_component()
                    .insert(world.entities().entity(0), self.root)
                    .expect("can write the root block");
                world
            }
            // If something is holding onto a reference, that would be a leak
            // Safer to panic here and correct the code that is holding onto the reference
            Err(_) => panic!("There should be no dangling references to world"),
        }
    }
}

/// Public APIs
///
impl Parser {
    /// Creates a new parser and empty specs world
    ///
    /// The parser will create a new entity per block parsed.
    ///
    pub fn new() -> Self {
        Self::new_with(World::new())
    }

    /// Creates a new parser w/ world
    ///
    pub fn new_with(mut world: World) -> Self {
        world.register::<Block>();
        world.register::<BlockIndex>();
        world.register::<BlockProperties>();

        // entity 0 is reserved for the root block
        world.entities().create();

        let world = Arc::new(world);
        Self {
            world,
            index: HashMap::new(),
            root: Block::default(),
            blocks: BTreeMap::new(),
            parsing: None,
            custom_attributes: vec![],
            parser_stack: vec![],
            implicit_block_symbol: None,
            default_custom_attribute: None,
            default_property_attribute: None,
            enabled: false,
            metadata: Default::default(),
        }
    }

    /// Sets the default custom attribute,
    ///
    pub fn set_default_custom_attribute(&mut self, custom: CustomAttribute) {
        self.default_custom_attribute = Some(custom);
    }

    /// Sets the default property attribute,
    ///
    pub fn set_default_property_attribute(&mut self, property: PropertyAttribute) {
        self.default_property_attribute = Some(property);
    }

    /// Sets the current metadata,
    /// 
    pub fn set_metadata(&mut self, metadata: Metadata) {
        self.metadata = metadata;
    }

    /// Sets the implicit symbol for the parser,
    ///
    pub fn set_implicit_symbol(&mut self, symbol: impl Into<String>) {
        self.implicit_block_symbol = Some(symbol.into());
    }

    /// Unset this so that block search works like normal,
    ///
    pub fn unset_implicit_symbol(&mut self) {
        self.implicit_block_symbol = None;
    }

    /// Includes a special attribute with this parser,
    ///
    /// Caveat - If multiple special attribute types share the same identifier,
    /// the last one added will be used.
    ///
    pub fn with_special_attr<S>(mut self) -> Self
    where
        S: SpecialAttribute,
    {
        self.add_custom_attribute(CustomAttribute::new::<S>());
        self
    }

    /// Adds a custom attribute parser,
    ///
    pub fn add_custom_attribute(&mut self, custom: CustomAttribute) {
        self.custom_attributes.push(custom);
    }

    /// Returns an immutable ref to the World
    ///
    pub fn world(&self) -> Arc<World> {
        self.world.clone()
    }

    /// Consumes self and returns the current world,
    ///
    /// Writes all blocks from this parser to world before returning the world.
    ///
    /// Panics if there are existing references to World
    ///
    pub fn commit(mut self) -> World {
        self.evaluate_stack();

        self.blocks
            .insert(self.world.entities().entity(0), self.root.clone());

        for (entity, block) in self.blocks.iter() {
            match self
                .world
                .write_storage()
                .insert(entity.clone(), block.clone())
            {
                Ok(_) => {
                    event!(
                        Level::DEBUG,
                        "Committing block {} {} @ {:?}",
                        block.name(),
                        block.symbol(),
                        entity
                    );
                }
                Err(err) => {
                    event!(
                        Level::ERROR,
                        "Could not commit block {} {} @ {:?}\n\t{err}",
                        block.name(),
                        block.symbol(),
                        entity
                    )
                }
            }

            let control_block = if !block.is_control_block() {
                self.index
                    .get(&format!(" {}", block.symbol()))
                    .and_then(|e| self.blocks.get(e))
            } else {
                None
            };

            for index in block.index() {
                for (child, properties) in index.iter_children() {
                    let child = self.world.entities().entity(*child);
                    let mut block_index = index.clone();
                    let mut properties = properties.clone();

                    if let Some(control_block) = control_block {
                        for (name, value) in control_block.map_control() {
                            properties.add(name.to_string(), value.clone());
                            block_index.add_control(name.clone(), value.clone());
                        }
                    }

                    for (name, value) in block.map_control() {
                        properties.add(name.to_string(), value.clone());
                        block_index.add_control(name.clone(), value.clone());
                    }

                    self.world
                        .write_component()
                        .insert(child, properties)
                        .expect("should be able to insert block properties");

                    self.world
                        .write_component()
                        .insert(child, block_index)
                        .expect("should be able to insert block index");

                    self.world
                        .write_component()
                        .insert(child, block.clone())
                        .expect("should be able to insert block index");

                    event!(
                        Level::DEBUG,
                        "Committing block properties for child entity {:?}",
                        child
                    );
                }
            }
        }

        self.into()
    }

    /// Returns a immuatble reference to the root block
    ///
    pub fn root(&self) -> &Block {
        &self.root
    }

    /// Returns a mutable reference to the root block
    ///
    pub fn root_mut(&mut self) -> &mut Block {
        &mut self.root
    }

    /// Parses .runmd content, updating internal state, and returns self
    ///
    pub fn parse(self, content: impl AsRef<str>) -> Self {
        let mut lexer = Keywords::lexer_with_extras(content.as_ref(), self);

        while let Some(token) = lexer.next() {
            match token {
                Keywords::NewLine => {
                    lexer.extras.metadata.move_col(1);
                    event!(
                        Level::TRACE,
                        "{} Parsed token, {:?}",
                        lexer.extras.metadata,
                        token,
                    );
                    lexer.extras.metadata.new_line();
                }
                _ if lexer.slice().contains("\n") => {
                    lexer.extras.metadata.move_col(lexer.slice().trim_end().len());
                    event!(
                        Level::TRACE,
                        "{} Parsed token, {:?} {}",
                        lexer.extras.metadata,
                        token,
                        lexer.slice()
                    );
                    lexer.extras.metadata.new_line();
                }
                _ => {
                    lexer.extras.metadata.move_col(lexer.slice().trim_end().len());
                    event!(
                        Level::TRACE,
                        "{} Parsed token, {:?} {}",
                        lexer.extras.metadata,
                        token,
                        lexer.slice()
                    );
                }
            }
        }

        lexer.extras
    }

    /// Gets a block from the parser
    ///
    pub fn get_block(&mut self, name: impl AsRef<str>, symbol: impl AsRef<str>) -> &Block {
        let block = self.ensure_block(name, symbol);

        self.blocks
            .get(&block)
            .expect("lookup block should have created a new block")
    }

    /// Returns an iterator over all blocks parsed
    ///
    pub fn iter_blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter().map(|(_, b)| b)
    }

    /// Creates and returns a new attribute parser,
    ///
    /// Will set the id to the current block entity.
    /// This parser will be at the top of the stack.
    ///
    pub fn new_attribute(&mut self) -> &mut AttributeParser {
        let mut attr_parser = AttributeParser::default();

        attr_parser.set_world(self.world.clone());

        for custom_attr in self.custom_attributes.iter().cloned() {
            trace!(
                "Adding custom attr parser, {}",
                custom_attr.ident()
            );
            attr_parser.add_custom(custom_attr);
        }

        if let Some(default_custom) = self.default_custom_attribute.as_ref() {
            attr_parser.set_default_custom_attribute(default_custom.clone());
        }

        if let Some(default_property) = self.default_property_attribute.as_ref() {
            attr_parser.set_default_property_attribute(default_property.clone());
        }

        attr_parser.set_id(self.parsing.and_then(|p| Some(p.id())).unwrap_or(0));
        attr_parser.set_block_identifier(self.current_block().identifier());
        self.parser_stack.push(attr_parser.clone());
        self.parser_stack.last_mut().expect("just added")
    }

    /// Consumes the current stack of attribute parsers, adding them to the
    /// current block being parsed.
    ///
    pub fn evaluate_stack(&mut self) {
        while let Some(mut attr_parser) = self.parser_stack.pop() {
            let attr_parser = &mut attr_parser;

            while let Some(attr) = attr_parser.next() {
                self.current_block().add_attribute(&attr);
            }
        }
    }
}

/// Private APIs
///
impl Parser {
    /// Gets a block by name/symbol, if it doesn't already exist, creates and indexes a new block
    ///
    fn ensure_block(&mut self, name: impl AsRef<str>, symbol: impl AsRef<str>) -> Entity {
        let mut name = name.as_ref();
        let mut symbol = symbol.as_ref();

        match self.implicit_block_symbol.as_ref() {
            Some(implicit_symbol) if symbol.is_empty() && name.is_empty() => {
                // Case root block -> control block
                symbol = implicit_symbol.as_str();
            }
            Some(implicit_symbol) if name.is_empty() && !symbol.is_empty() => {
                // Case control block -> event block
                name = symbol;
                symbol = implicit_symbol.as_str();
            }
            _ => {
                // No-op
            }
        }

        let key = format!("{name} {symbol}");
        event!(Level::TRACE, "Parsing block {key}");
        match self.index.get(&key) {
            Some(block) => *block,
            None => {
                let entity = self.world.entities().create();
                let block = Block::new(entity, name, symbol);

                self.blocks.insert(entity, block);
                self.index.insert(key, entity);
                entity
            }
        }
    }

    /// Returns the current block being built
    ///
    fn current_block(&mut self) -> &mut Block {
        if let Some(parsing) = self.parsing {
            self.blocks.get_mut(&parsing).expect("should be a block")
        } else {
            &mut self.root
        }
    }

    /// Returns the current block symbol
    ///
    fn current_block_symbol(&self) -> String {
        if let Some(parsing) = self.parsing.as_ref() {
            self.blocks
                .get(parsing)
                .expect("should be a block")
                .symbol()
                .to_string()
        } else {
            String::default()
        }
    }

    /// Returns the last attribute parser so additional
    /// transient properties can be added.
    ///
    /// If the last attribute parser parsed a special attribute,
    /// new special attributes could've been enabled for subsequent property
    /// definitions.
    ///
    fn parse_property(&mut self) -> &mut AttributeParser {
        if !self.parser_stack.is_empty() {
            self.parser_top().expect(
                "should return a parser since we check if the stack is empty before this line",
            )
        } else {
            self.new_attribute()
        }
    }

    /// Returns the current attribute parser,
    ///
    fn parser_top(&mut self) -> Option<&mut AttributeParser> {
        self.parser_stack
            .last_mut()
    }
}

impl AsMut<AttributeParser> for Parser {
    fn as_mut(&mut self) -> &mut AttributeParser {
        self.parser_top()
            .expect("should have an attribute parser at the top")
    }
}

#[allow(unused_imports)]
#[allow(dead_code)]
mod tests {
    use logos::Logos;

    use crate::{Keywords, Parser, Value};

    const TEST_CONTENT: &'static str = r#"
    # Test 
    This to test that the enabled disabled is working.

    ``` call host 
    <>
    + address .text localhost 
    : ipv6 .enable
    : path .text api/test 
    : name .text test_name
    ``` guest 
    + address .text localhost
    : ipv4 .enable
    : path .text api/test2
    ```

    ``` test host 
    + address .text localhost
    ``` 

    ```
    + debug        .enable  
    + test         .empty  Everything after this is ignored when parsed 
    : name         .text   Test map 
    : description  .text   This tests the .map type, which is an alias for .empty 
    ``` guest
    : name .text cool guest host
    + address .text testhost
    ```


    ```
    +  inline_person .int 99 : name .text John : age .int 99 : weight .float 3.14 : real .false
    ```
    "#;

    #[tracing_test::traced_test]
    #[test]
    fn test_lexer() {
        use crate::Value;

        // Tests the lexer logic
        let parser = Parser::new();
        let mut lexer = Keywords::lexer_with_extras(TEST_CONTENT, parser);
        // let skip = lexer.source().find("```").unwrap();
        // lexer.bump(skip);
        /*
         ``` call host
        add address .text localhost
        :: ipv6 .enable
        :: path .text api/test
        ``` guest
        + address .text localhost
        :: ipv4 .enable
        :: path .text api/test2
        ```
        */
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Extension));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Add));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(
            lexer.next(),
            Some(Keywords::Define),
            "slice -- {}, {}",
            lexer.slice(),
            lexer.remainder()
        );
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Add));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(
            lexer.next(),
            Some(Keywords::Define),
            "slice -- {}, {}",
            lexer.slice(),
            lexer.remainder()
        );
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));

        /*
        ``` test host
        add address .text localhost
        ```
        */
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Add));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));

        /*
        ```
        + debug .enable
        + test          .map    Everything after this is ignored when parsed
        :: name         .text   Test map
        :: description  .text   This tests the .map type, which is an alias for .empty
        ``` guest
        :: name .text cool guest host
        + address .text testhost
        ```
        */
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Add));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Add));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::Add));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));

        /*
        ```
        +  inline_person .text John : age .int 99 : weight .float 3.14 : real .false
        ```
        */
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
        assert_eq!(lexer.next(), Some(Keywords::NewLine));
        assert_eq!(lexer.next(), Some(Keywords::Add));
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::Define));
        assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    }

    #[tracing_test::traced_test]
    #[test]
    fn test_parse() {
        // Tests parsing logic
        let mut parser = Parser::new().parse(TEST_CONTENT);

        let address = parser.get_block("call", "host").map_transient("address");
        assert_eq!(address.get("ipv6"), Some(&Value::Bool(true)));
        assert_eq!(
            address.get("path"),
            Some(&Value::TextBuffer("api/test".to_string()))
        );
        assert_eq!(
            address.get("name"),
            Some(&Value::TextBuffer("test_name".to_string()))
        );

        let address = parser.get_block("call", "guest").map_transient("address");
        assert_eq!(
            address.get("ipv4"),
            Some(&Value::Bool(true)),
            "{:?}",
            address
        );
        assert_eq!(
            address.get("path"),
            Some(&Value::TextBuffer("api/test2".to_string()))
        );

        let guest = parser.get_block("call", "guest").map_stable();
        assert_eq!(
            guest.get("address"),
            Some(&Value::TextBuffer("localhost".to_string()))
        );
        assert_eq!(
            parser.root().map_stable().get("debug"),
            Some(&Value::Bool(true))
        );

        // Tests .map alias
        assert_eq!(
            parser.root().map_transient("test").get("name"),
            Some(&Value::TextBuffer("Test map".to_string())),
            "{:#?}",
            parser.root()
        );

        let root_guest = parser.get_block("", "guest").map_stable();
        assert_eq!(
            root_guest.get("address"),
            Some(&Value::TextBuffer("testhost".to_string()))
        );

        let root_guest_control = parser.get_block("", "guest").map_control();
        assert_eq!(
            root_guest_control.get("name"),
            Some(&Value::TextBuffer("cool guest host".to_string())),
            "{:#?}\n{:#?}",
            root_guest_control,
            parser
                .get_block("", "guest")
                .iter_attributes()
                .collect::<Vec<_>>(),
        );

        let inline_test = parser.root().map_transient("inline_person");
        assert_eq!(
            inline_test
                .get("name")
                .expect("should have name")
                .text()
                .unwrap()
                .as_str(),
            "John"
        );
        assert_eq!(
            inline_test
                .get("age")
                .expect("should have age")
                .int()
                .unwrap(),
            99
        );
        assert_eq!(
            inline_test
                .get("weight")
                .expect("should have weight")
                .float()
                .unwrap(),
            3.14
        );
        assert_eq!(
            inline_test
                .get("real")
                .expect("should have real")
                .bool()
                .unwrap(),
            false
        );
    }

    use specs::WorldExt;

    use crate::SpecialAttribute;

    struct TestChild;

    impl SpecialAttribute for TestChild {
        fn ident() -> &'static str {
            "test_child"
        }

        fn parse(parser: &mut crate::AttributeParser, _: impl AsRef<str>) {
            let child = parser
                .world()
                .expect("should have a world")
                .entities()
                .create();
            parser.define_child(child, "is_child", true);
        }
    }

    #[test]
    #[tracing_test::traced_test]
    fn test_implicit_symbols() {
        use crate::BlockIndex;
        use crate::Parser;
        use crate::Value;

        let _content = r#"
        ```
        <test inline comment>  : domain .symbol test
    
        <test inline comment>  + address .symbol localhost
        ```
    
        ``` event-1
        <test inline comment> + name .symbol event-1
        ```
    
        ``` event-2
        <test inline comment>  + .test_child

        <test inline comment> + name .symbol event-2
        ```
        "#;

        let mut parser = Parser::new().with_special_attr::<TestChild>();
        parser.set_implicit_symbol("test");

        let mut parser = parser.parse(_content);
        parser.unset_implicit_symbol();

        let max = parser.blocks.iter().max_by_key(|k| k.0.id());
        eprintln!("{:#?}", max);

        let block = parser.get_block("", "test");
        let address = block.map_stable();
        let address = address
            .get("address")
            .expect("should have address stable attr");
        assert_eq!(address, &Value::Symbol("localhost".to_string()));

        let block = parser.get_block("event-1", "test");
        let name = block.map_stable();
        let name = name.get("name").expect("should have name stable attr");
        assert_eq!(name, &Value::Symbol("event-1".to_string()));

        let block = parser.get_block("event-2", "test");
        let name = block.map_stable();
        let name = name.get("name").expect("should have name stable attr");
        assert_eq!(name, &Value::Symbol("event-2".to_string()));

        let world = parser.commit();

        let domain = world
            .read_component::<BlockIndex>()
            .get(world.entities().entity(4))
            .expect("should have a block")
            .clone();

        let domain = domain
            .control_values()
            .get("domain")
            .expect("should have a value");

        assert_eq!(domain, &Value::Symbol("test".to_string()));
    }
}
