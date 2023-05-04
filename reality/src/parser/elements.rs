use logos::{Lexer, Logos};

/// In .runmd, blocks are identified w/ a name/symbol pair,
///
/// # Background
///
/// In .md code blocks are delimitted by 3 ticks, and can have an identifier
/// to identify the language of code. This concept is the basis of blocks in .runmd.
///
/// # Identifying blocks
///
/// A block is identified by 3 ticks followed by a space and two identifiers (name and symbol respectively)
/// seperated by a space. The end of a block is 3 ticks with no following identifiers.
/// Within a block, a seperator w/ 3 ticks followed by one identifier, declares a new block
/// that reuses the same name identifier from the start of the block.
///
/// ## Special Case: Root Block
/// A block that begins and ends w/ no identifiers is referred to as the root block.
/// The root block is always entity 0.
///
/// *Note* Even though a inner block can be declared within a root block, that block
/// will be allocated to a seperate entity, and will not write values to the root block.
///
#[derive(Logos, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Elements {
    /// Escaped colon character
    /// 
    #[token("\\:")]
    EscapedColon,
    /// Inline operator
    /// 
    #[token(":")]
    InlineOperator,
    /// Identifier string, this is either a name or symbol
    ///
    #[regex("[./A-Za-z0-9]+[A-Za-z-._:=/#0-9]*", on_identifier)]
    Identifier(String),
    /// Comment,
    /// 
    #[token("/*", on_comment_start)]
    #[token("#", on_doc_comment_start)]
    Comment(String),
    /// New line,
    /// 
    #[token("\r")]
    #[token("\n")]
    #[token("\r\n")]
    NewLine,
    /// Valid token elements,
    /// 
    #[token("-")]
    #[token("{")]
    #[token("}")]
    Valid,
    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ ,\t\f]+", logos::skip)]
    Error,
}

impl Elements {
    /// Returns an ident element if valid ident,
    /// 
    pub fn ident(ident: impl AsRef<str>) -> Option<Elements> {
        match Elements::lexer(ident.as_ref()).next() {
            Some(ident) => match ident {
                Elements::Identifier(_) => Some(ident),
                _ => None
            },
            None => None,
        }
    }
}

fn on_identifier(lexer: &mut Lexer<Elements>) -> Option<String> {
    let slice = lexer.slice();
    Some(slice.to_string())
}

fn on_comment_start(lexer: &mut Lexer<Elements>) -> Option<String> {
    let end_pos = lexer.remainder()
        .lines()
        .take(1)
        .next()
        .and_then(|s| s.find("*/"))
        .expect("Didn't find a closing `*/`");
    
    let result = &lexer.remainder()[..end_pos];
    
    lexer.bump(end_pos + 2);

    Some(result.to_string())
}

fn on_doc_comment_start(lexer: &mut Lexer<Elements>) -> Option<String> {
    let end_pos = lexer.remainder()
        .lines()
        .take(1)
        .next()
        .map(|l| l.len())
        .unwrap_or_default();

    if end_pos > 0 {
        let result = &lexer.remainder()[..end_pos];
        lexer.bump(end_pos);
        Some(result.to_string())
    } else {
        let result = &lexer.remainder();
        lexer.bump(result.len());
        Some(result.to_string())
    }
}

#[test]
fn test_elements() {
    let test_str = ".Custom test value";

    assert_eq!(
        Elements::lexer(test_str).next().expect("parses"),
        Elements::Identifier(".Custom".to_string())
    );

    let test_str = "test, one, two, three /* test one two three */ # this is a doc comment";
    let mut lexer = Elements::lexer(test_str);
    assert_eq!(lexer.next(), Elements::ident("test"));
    assert_eq!(lexer.next(), Elements::ident("one"));
    assert_eq!(lexer.next(), Elements::ident("two"));
    assert_eq!(lexer.next(), Elements::ident("three"));
    assert_eq!(lexer.next(), Some(Elements::Comment(" test one two three ".to_string())));
    assert_eq!(lexer.next(), Some(Elements::Comment(" this is a doc comment".to_string())));
}
