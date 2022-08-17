use std::collections::{BTreeMap, HashMap};

use atlier::system::Attribute;
use specs::{Entity, World, WorldExt};
use logos::{Lexer, Logos};
use tracing::{event, Level};

use crate::Block;

mod attribute;
pub use attribute::AttributeParser;

mod block_ident;
use block_ident::BlockIdentity;

/// Parser for runmd using a world for storage 
///
/// Creates a new entity for each seperate block parsed. 
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

        self.blocks.get(&block).expect("lookup block should have created a new block")
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
    fn parse_add(&mut self) {
        let last = self.last_add.clone().expect("should be set");

        self.current_block().add_attribute(&last);
    }
    
    /// Parses the last attribute, adding it to the current block being build
    ///
    fn parse_define(&mut self) {
        let last = self.last_define.clone().expect("should be set");

        self.current_block().add_attribute(&last);
    }
}

#[derive(Logos, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[logos(extras = Parser)]
enum Keywords {
    /// Block delimitter either starts or ends a block
    ///
    /// If starting a block, the delimitter can be followed by two
    /// symbols representing the block name and block symbol.
    ///
    /// If this is the start of a block, with no name/symbol, then
    /// the root block will be used as the context.
    ///
    #[token("```", on_block_delimitter)]
    BlockDelimitter,
    /// Write a stable attribute
    ///
    #[token("add", on_add)]
    #[token("+", on_add)]
    Add,
    /// Writes a transient attribute
    ///
    /// If `::` is used, the current attribute parser will be reused.
    ///
    #[token("define", on_define)]
    #[token("::", on_define)]
    Define,
    /// Comments are skipped, usually .md list element or header so that the .runmd can be
    /// partially cross compatible w/ .md.
    ///
    #[token("#")]
    #[token("*")]
    #[token("-")]
    #[token("//")]
    #[token("``` md")]
    #[token("``` runmd")]
    Comment,
    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

fn on_block_delimitter(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        let mut block_ident = BlockIdentity::lexer(next_line);

        match (block_ident.next(), block_ident.next()) {
            (Some(BlockIdentity::Symbol(name)), Some(BlockIdentity::Symbol(symbol))) => {
                let current = lexer.extras.lookup_block(name, symbol);
                lexer.extras.parsing = Some(current);
            }
            (Some(BlockIdentity::Symbol(symbol)), _) => {
                let name = lexer.extras.current_block().name().to_string();
                let current = lexer.extras.lookup_block(name, symbol);
                lexer.extras.parsing = Some(current);
            }
            _ => {
                // If an ident is not set, then
                lexer.extras.parsing = None;
            }
        }
        lexer.bump(next_line.len());
    }
}

fn on_add(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        let mut attr_parser = AttributeParser::default();
        attr_parser.set_id(lexer.extras.parsing.and_then(|p| Some(p.id())).unwrap_or(0));
        lexer.extras.last_add = attr_parser.parse(next_line.trim()).add();
        lexer.extras.parse_add();
        lexer.bump(next_line.len());
    }
}

fn on_define(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        let mut attr_parser = AttributeParser::default();
        attr_parser.set_id(lexer.extras.parsing.and_then(|p| Some(p.id())).unwrap_or(0));

        // Syntax sugar for,
        // From -
        // add connection .empty
        // define connection host .text example.com
        // Sugar -
        // add connection .empty
        // :: host .text example.com
        //
        if lexer.slice() == "::" {
            let name = &lexer
                .extras
                .last_add
                .as_ref()
                .expect("Expected an attribute to have been parsed.")
                .name;
            attr_parser.set_name(name);
        }
        lexer.extras.last_define = attr_parser.parse(next_line.trim()).define();
        lexer.extras.parse_define();
        lexer.bump(next_line.len());
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
    + debug .enable 
    ```
    "#; 

    let parser = Parser::new();
    let mut lexer = Keywords::lexer_with_extras(content, parser);

    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    assert_eq!(lexer.next(), Some(Keywords::Add));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    assert_eq!(lexer.next(), Some(Keywords::Add));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::Define));
    assert_eq!(lexer.next(), Some(Keywords::BlockDelimitter));
    eprintln!("{:#?}", lexer.extras.blocks);
    
    let mut parser = Parser::new().parse(content);

    let address = parser
        .get_block("call", "guest")
        .map_transient("address");
    
    assert_eq!(
        address.get("ipv4"), 
        Some(&Value::Bool(true))
    );

    assert_eq!(
        address.get("path"), 
        Some(&Value::TextBuffer("api/test2".to_string()))
    );

    let guest = parser
        .get_block("call", "guest")
        .map_stable();
    
    assert_eq!(
        guest.get("address"), 
        Some(&Value::TextBuffer("localhost".to_string()))
    );

    assert_eq!(
        parser.root().map_stable().get("debug"), 
        Some(&Value::Bool(true))
    );
}
