use super::prelude::*;

/// Enumeration of instructions emitted by runmd,
///
#[derive(Hash, Logos, Default, Debug, Clone, PartialEq)]
#[logos(skip r"[\n\r ]*")]
#[logos(extras = Context<'s>)]
pub enum Instruction {
    /// Start of a block,
    ///
    #[token("```runmd", on_block_start)]
    BlockStart,
    /// Adds a node to the block,
    ///
    #[token("+", on_add_node)]
    AddNode,
    /// Defines a property for a block or node,
    ///
    #[token(":", on_define_property)]
    DefineProperty,
    /// Loads an extension for a block or node,
    ///
    #[token("<", on_load_extension)]
    LoadExtension,
    /// Loads an extension by suffix based on the previously loaded extension for a block or node,
    ///
    #[token("<..", on_load_extension_suffix)]
    LoadExtensionSuffix,
    /// Appends input to the last line,
    /// 
    #[token("|", on_append_input)]
    AppendInput,
    /// End of a block,
    ///
    #[token("```", on_block_end)]
    BlockEnd,
    /// Noop,
    ///
    #[default]
    Noop,
    /// The ignored portion of the source text,
    /// 
    #[regex(r"[^`+:<|]+", on_ignored)]
    Ignored,
}

/// Returns the next token if it is the next token,
///
macro_rules! next_if_token {
    ($peekable:ident, $variant:ident, $parse:ident) => {
        if $peekable
            .peek()
            .filter(|p| {
                p.as_ref()
                    .ok()
                    .filter(|p| matches!(p, Tokens::$variant(..)))
                    .is_some()
            })
            .is_some()
        {
            $peekable
                .next()
                .and_then(Result::ok)
                .and_then(Tokens::$parse)
        } else {
            None
        }
    };
}

#[inline]
fn on_ignored(lex: &mut Lexer<Instruction>) -> Filter<()> {
    if lex.extras.is_analyzing() {
        Filter::Skip
    } else {
        Filter::Emit(())
    }
}

fn on_block_start(lex: &mut Lexer<Instruction>) {
    lex.extras.start_block();

    // Parse the optional identifier
    if let Some(ident) = lex.remainder().lines().next() {
        let mut parts = ident.split_whitespace();

        match (parts.next(), parts.next()) {
            (Some(ty), Some(moniker)) => {
                lex.extras.set_block_ty(ty);
                lex.extras.set_block_moniker(moniker);
            },
            (Some(ty), None) => {
                lex.extras.set_block_ty(ty);
            }
            _ => {

            }
        }

        lex.bump_line();
    }
}

#[inline]
fn on_block_end(lex: &mut Lexer<Instruction>) {
    lex.extras.end_block();
}

#[inline]
fn on_append_input(lex: &mut Lexer<Instruction>) -> Filter<()> {
    if lex.extras.is_analyzing() {
        if let Some(input) = lex.bump_line() {
            lex.extras.append_input(input.trim_start_matches('|').trim());
        }
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

#[inline]
fn on_add_node(lex: &mut Lexer<Instruction>) -> Filter<()> {
    if lex.extras.is_analyzing() {
        lex.extras.set_instruction(Instruction::AddNode);
        on_attribute(lex);
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

#[inline]
fn on_define_property(lex: &mut Lexer<Instruction>) -> Filter<()> {
    if lex.extras.is_analyzing() {
        lex.extras.set_instruction(Instruction::DefineProperty);
        on_attribute(lex);
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

#[inline]
fn on_load_extension(lex: &mut Lexer<Instruction>) -> Filter<()> {
    if lex.extras.is_analyzing() {
        lex.extras.set_instruction(Instruction::LoadExtension);
        on_extension(lex);
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

#[inline]
fn on_load_extension_suffix(lex: &mut Lexer<Instruction>) -> Filter<()> {
    if lex.extras.is_analyzing() {
        lex.extras.set_instruction(Instruction::LoadExtensionSuffix);
        on_extension(lex);
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

/// Parses the parameters of an attribute container,
/// 
fn on_attribute(lex: &mut Lexer<Instruction>) {
    // Morph into tokens lexer
    let tokens: Lexer<Tokens> = lex.clone().morph();

    // Tokenize line
    let mut peekable = tokens.peekable();
    lex.extras.add_line(Line {
        tag: next_if_token!(peekable, Tag, parse_tag),
        attr: next_if_token!(peekable, Attribute, parse_attr),
        comment: next_if_token!(peekable, Comment, parse_comment),
        ..Default::default()
    });
    lex.bump_line();
}

/// Parses the parameters of an extension container,
/// 
fn on_extension(lex: &mut Lexer<Instruction>) {
    // Morph into tokens lexer
    let mut tokens: Lexer<Tokens> = lex.clone().morph();

    // Tokenize line
    let mut peekable = tokens.by_ref().peekable();
    let extension = next_if_token!(peekable, Extension, parse_extension);

    if extension.is_none() {
        // Add error handling
        panic!("Should be currently tokenizing an extension statement");
    }

    lex.extras.add_line(Line {
        extension,
        comment: next_if_token!(peekable, Comment, parse_comment),
        ..Default::default()
    });
    lex.bump_line();
}

#[test]
fn test_add_node_instruction() {
    let mut context = Context::default();
    context.start_block();

    let mut lex = Instruction::lexer_with_extras(
        r"
    + example .test 'hello-world' # Test comment
    + .test hello world 2 # test comment
    + .test hello world 3 # test comment
    | 4 
    | 5 
    | 6 test 12345 
    ",
        context,
    );

    assert_eq!(lex.next(), Some(Ok(Instruction::AddNode)));
    assert_eq!(lex.next(), Some(Ok(Instruction::AddNode)));
    assert_eq!(lex.next(), Some(Ok(Instruction::AddNode)));
    assert_eq!(lex.next(), Some(Ok(Instruction::AppendInput)));
    assert_eq!(lex.next(), Some(Ok(Instruction::AppendInput)));
    assert_eq!(lex.next(), Some(Ok(Instruction::AppendInput)));
    lex.extras.end_block();

    let block = lex.extras.blocks.pop().expect("should have a block");

    let line = &block.lines[0];
    assert_eq!(line.tag, Some(Tag("example")));
    assert_eq!(line.attr, Some(Attribute { name: "test", input: Some(Input::EscapedText("hello-world")) }));
    assert_eq!(line.comment, Some("# Test comment"));
    assert!(line.extension.is_none());

    let line = &block.lines[1];
    assert_eq!(line.attr, Some(Attribute { name: "test", input: Some(Input::Text("hello world 2")) }));
    assert_eq!(line.comment, Some("# test comment"));
    assert!(line.extension.is_none());
    assert!(line.tag.is_none());

    let line = &block.lines[2];
    assert_eq!(line.attr, Some(Attribute { name: "test", input: Some(Input::Lines(vec!["hello world 3", "4", "5", "6 test 12345"])) }));
    assert_eq!(line.comment, Some("# test comment"));
    assert!(line.extension.is_none());
    assert!(line.tag.is_none());
}
