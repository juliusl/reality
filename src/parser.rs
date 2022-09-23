use specs::storage::GenericWriteStorage;
use specs::{Entity, World, WorldExt};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tracing::{event, Level};

use logos::Logos;

use crate::Block;

mod attributes;
pub use attributes::AttributeParser;
pub use attributes::Attributes;
pub use attributes::BlobDescriptor;
pub use attributes::File;
pub use attributes::CustomAttribute;
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
                world.insert(self.index);
                world.insert(self.custom_attributes);
                
                world.write_component().insert(
                    world.entities().entity(0), 
                    self.root
                ).expect("can write the root block");
                world
            },
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
        let mut world = World::new();
        world.register::<Block>();

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
            parser_stack: vec![]
        }
    }

    /// Includes a special attribute with this parser,
    /// 
    /// Caveat - If multiple special attribute types share the same identifier,
    /// the last one added will be used. 
    /// 
    pub fn with_special_attr<S>(mut self) -> Self 
    where
        S: SpecialAttribute
    {
        self.custom_attributes.push(CustomAttribute::new::<S>());
        self
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

                    for index in block.index() {
                        for (child, properties) in index.iter_children() {
                            let child = self.world.entities().entity(*child);
                            
                            self.world.write_component()
                                .insert(child, properties.clone())
                                .expect("could not add component for child entity");
                            
                            event!(
                                Level::DEBUG, 
                                "Committing block properties for child entity {:?}", 
                                child
                            );
                        }
                    }
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
            event!(Level::TRACE, "Parsed token, {:?}", token);
        }

        lexer.extras
    }

    /// Gets a block from the parser
    ///
    pub fn get_block(&mut self, name: impl AsRef<str>, symbol: impl AsRef<str>) -> &Block {
        let block = self.lookup_block(name, symbol);

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
        let mut attr_parser = AttributeParser::default()
            .with_custom::<File>()
            .with_custom::<BlobDescriptor>();
        
        attr_parser.set_world(self.world.clone());
        
        for custom_attr in self.custom_attributes.iter().cloned() {
            event!(Level::TRACE, "Adding custom attr parser, {}", custom_attr.ident());
            attr_parser.add_custom(custom_attr);
        }
        
        attr_parser.set_id(self.parsing.and_then(|p| Some(p.id())).unwrap_or(0));
        self.parser_stack.push(attr_parser);
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
    fn lookup_block(&mut self, name: impl AsRef<str>, symbol: impl AsRef<str>) -> Entity {
        let name = name.as_ref();
        let symbol = symbol.as_ref();

        let key = format!("{name} {symbol}");
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
            self.blocks.get(parsing).expect("should be a block").symbol().to_string()
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
            self.parser_top().unwrap()
        } else {
            self.new_attribute()
        }
    }

    fn parser_top(&mut self) -> Option<&mut AttributeParser> {
        self.parser_stack.last_mut()
    }
}

#[test]
fn test_parser() {
    use atlier::system::Value;

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
    + debug         .enable  
    + test          .map    Everything after this is ignored when parsed 
    :: name         .text   Test map 
    :: description  .text   This tests the .map type, which is an alias for .empty 
    
    ``` guest
    :: name .text cool guest host
    + address .text testhost
    ```
    "#;

    // Tests the lexer logic
    let parser = Parser::new();
    let mut lexer = Keywords::lexer_with_extras(content, parser);

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
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    assert_eq!(lexer.next(), Some(Keywords::Add));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    assert_eq!(lexer.next(), Some(Keywords::Add));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));

    /*
    ``` test host
    add address .text localhost
    ```
    */
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    assert_eq!(lexer.next(), Some(Keywords::Add));
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
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    assert_eq!(lexer.next(), Some(Keywords::Add));
    assert_eq!(lexer.next(), Some(Keywords::Add));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::Add));
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));

    // Tests parsing logic
    let mut parser = Parser::new().parse(content);

    let address = parser.get_block("call", "host").map_transient("address");
    assert_eq!(address.get("ipv6"), Some(&Value::Bool(true)));
    assert_eq!(
        address.get("path"),
        Some(&Value::TextBuffer("api/test".to_string()))
    );

    let address = parser.get_block("call", "guest").map_transient("address");
    assert_eq!(address.get("ipv4"), Some(&Value::Bool(true)), "{:?}", address);
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
        root_guest_control
            .as_ref()
            .expect("should have control block")
            .get("name"),
        Some(&Value::TextBuffer("cool guest host".to_string())),
        "{:#?}\n{:#?}",
        root_guest_control,
        parser
            .get_block("", "guest")
            .iter_attributes()
            .collect::<Vec<_>>(),
    );
}
