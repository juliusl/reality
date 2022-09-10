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
    /// Identifier string, this is either a name or symbol
    ///
    #[regex("[A-Za-z]+[A-Za-z-._:=/#0-9]*", on_identifier)]
    Identifier(String),
    #[token(".", on_attribute_type)]
    AttributeType(String),
    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

fn on_identifier(lexer: &mut Lexer<Elements>) -> Option<String> {
    let mut slice = lexer.slice();
    if slice.starts_with('#') {
        slice = &slice[1..];
    }

    Some(slice.to_string())
}

fn on_attribute_type(lexer: &mut Lexer<Elements>) -> Option<String> {
    match lexer.next() {
        Some(elem) => match elem {
            Elements::Identifier(ident) => Some(ident),
            _ => None,
        },
        None => None,
    }
}

#[test]
fn test_elements() {
    let test_str = ".Custom";

    assert_eq!(
        Elements::lexer(test_str).next().expect("parses"),
        Elements::AttributeType("Custom".to_string())
    );
}
