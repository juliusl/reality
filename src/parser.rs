use specs::{Entity, World, WorldExt};
use std::collections::{BTreeMap, HashMap};
use tracing::{event, Level};

use atlier::system::Attribute;
use logos::Logos;

use crate::Block;

mod attributes;
pub use attributes::AttributeParser;
pub use attributes::Attributes;
pub use attributes::BlobDescriptor;
pub use attributes::FileDescriptor;

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
    world: World,
    /// Root block
    root: Block,
    /// Reverse lookup entity from block name/symbol
    index: HashMap<String, Entity>,
    /// Named blocks
    blocks: BTreeMap<Entity, Block>,
    /// The current block being parsed
    parsing: Option<Entity>,
    /// The last `add` keyword parsed
    last_add: Option<Attribute>,
    /// The last `define` keyword parsed
    last_define: Option<Attribute>,
}

impl AsRef<World> for Parser {
    fn as_ref(&self) -> &World {
        &self.world
    }
}

impl AsMut<World> for Parser {
    fn as_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl Into<World> for Parser {
    fn into(self) -> World {
        self.world
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
        let world = World::new();

        // entity 0 is reserved for the root block
        world.entities().create();

        Self {
            world,
            index: HashMap::new(),
            root: Block::default(),
            blocks: BTreeMap::new(),
            parsing: None,
            last_add: None,
            last_define: None,
        }
    }

    /// Returns an immutable ref to the World
    ///
    pub fn world(&self) -> &World {
        self.as_ref()
    }

    /// Returns a mut reference to the World
    ///
    pub fn world_mut(&mut self) -> &mut World {
        self.as_mut()
    }

    /// Consumes self and returns the current world,
    ///
    /// Writes all blocks from this parser to world before returning the world.
    ///
    pub fn commit(mut self) -> World {
        self.world.register::<Block>();

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

    /// Returns an attribute parser,
    ///
    /// If the world has an memory blob source, it will be used
    /// to resolve `.blob` attributes
    ///
    pub fn attribute_parser(&self) -> AttributeParser {
        let mut attr_parser = AttributeParser::default()
            .with_custom::<FileDescriptor>()
            .with_custom::<BlobDescriptor>();
        attr_parser.set_id(self.parsing.and_then(|p| Some(p.id())).unwrap_or(0));
        attr_parser
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

    /// Parses the last attribute, adding it to the current block being build
    ///
    fn parse_define(&mut self) {
        let last = self.last_define.clone().expect("should be set");

        self.current_block().add_attribute(&last);
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
    assert_eq!(address.get("ipv4"), Some(&Value::Bool(true)));
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
