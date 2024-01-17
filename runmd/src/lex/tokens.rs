use super::prelude::*;

/// Variants of tokens recognized by runmd,
///
#[derive(Logos, Debug, Clone)]
#[logos(skip r"[\n\r ]*")]
#[logos(extras = Context<'s>)]
pub enum Tokens<'source> {
    /// Extension container,
    ///
    #[regex(
        "([a-zA-Z]*[/]?[a-zA-Z0-9._-]+|([(][a-zA-Z0-9._-]+[ ]*[a-zA-Z0-9._-]+[)]))>",
        on_extension
    )]
    Extension(Extension<'source>),
    /// Attribute container,
    ///
    #[regex("[.][a-zA-Z][a-zA-Z0-9_-]*", on_attribute)]
    Attribute(Attribute<'source>),
    /// Tag value,
    ///
    #[regex("[a-zA-Z][a-zA-Z0-9_-]+", on_tag)]
    Tag(Tag<'source>),
    /// Comment value,
    ///
    #[regex("[/][/][^\r\n]+")]
    #[regex("[#][^\r\n]+")]
    Comment(&'source str),
}

impl<'a> Tokens<'a> {
    /// Consumes self and returns as Extension or None
    ///
    #[inline]
    pub fn parse_extension(self) -> Option<Extension<'a>> {
        match self {
            Tokens::Extension(ext) => Some(ext),
            _ => None,
        }
    }

    /// Consumes self and returns as Tag or None
    ///
    #[inline]
    pub fn parse_tag(self) -> Option<Tag<'a>> {
        match self {
            Tokens::Tag(tag) => Some(tag),
            _ => None,
        }
    }

    /// Consumes self and returns as Attribute or None
    ///
    #[inline]
    pub fn parse_attr(self) -> Option<Attribute<'a>> {
        match self {
            Tokens::Attribute(attr) => Some(attr),
            _ => None,
        }
    }

    /// Consumes self and returns as Attribute or None
    ///
    #[inline]
    pub fn parse_comment(self) -> Option<Vec<&'a str>> {
        match self {
            Tokens::Comment(c) => Some(vec![c]),
            _ => None,
        }
    }
}

macro_rules! get_input {
    ($lex:ident) => {{
        // Morph into Input lexer
        let mut input: Lexer<Input> = $lex.clone().morph();

        // Check if there is input w/o changing the state of the underlying lexer
        let mut peekable = input.by_ref().peekable();
        let has_input = peekable.peek().is_some_and(Result::is_ok);

        let input_result = if has_input {
            let result = peekable.next().and_then(Result::ok);

            // Morph lexer back to update state
            // Note: This will eat the whitespace
            *$lex = input.morph();
            result
        } else {
            None
        };

        input_result
    }};
}

fn on_extension<'s>(lex: &mut Lexer<'s, Tokens<'s>>) -> Filter<Extension<'s>> {
    if !lex.extras.is_analyzing() {
        return Filter::Skip;
    }

    let result = if lex.extras.is_instruction_load_extension_suffix() {
        lex.extras
            .get_extension(Some(lex.slice().trim_end_matches('>')))
            .map(|mut e| {
                e.input = get_input!(lex);
                e
            })
            .expect("should have an extension currently set if loading by suffix")
    } else if lex.slice().contains('/') {
        if let Some(tag) = lex.slice().split('/').next() {
            Extension {
                tag: Some(tag),
                name: lex
                    .slice()
                    .trim_start_matches(tag)
                    .trim_start_matches('/')
                    .trim_end_matches('>'),
                suffix: None,
                input: get_input!(lex),
            }
        } else {
            Extension {
                tag: None,
                name: lex.slice().trim_end_matches('>'),
                suffix: None,
                input: get_input!(lex),
            }
        }
    } else {
        Extension {
            tag: if lex.slice().contains('/') {
                lex.slice().split('/').next()
            } else {
                None
            },
            name: lex.slice().trim_end_matches('>'),
            suffix: None,
            input: get_input!(lex),
        }
    };

    Filter::Emit(result)
}

fn on_attribute<'s>(lex: &mut Lexer<'s, Tokens<'s>>) -> Filter<Attribute<'s>> {
    if !lex.extras.is_analyzing() {
        Filter::Skip
    } else {
        // Morph into input parser
        let mut attr = Attribute {
            name: lex.slice().trim_start_matches('.'),
            input: None,
        };

        attr.input = get_input!(lex);
        Filter::Emit(attr)
    }
}

#[inline]
fn on_tag<'s>(lex: &mut Lexer<'s, Tokens<'s>>) -> Filter<Tag<'s>> {
    if !lex.extras.is_analyzing() {
        Filter::Skip
    } else {
        Filter::Emit(Tag(lex.slice()))
    }
}

#[test]
fn test_token_lexer() {
    let mut lex = Instruction::lexer(
        r"
    # Test runmd document
    + This is a test document.
    
    ```runmd application/test.block root
    + .test test/test.node          # Test adding a new node
    <application/test.extension>    # Test loading in an extension
    : .name-1 hello-world           # Test defining a property
    <..extension-2> testinput       # Test loading another extension by suffix
    : .name-1 hello-world-2         # Test defining a property
    : .name-2 'hello-world-3'       # Test defining a property

    + example .test test/test.node  # Test adding an additional node
    <application/test.extension>    # Test loading an extension
    <..example>     Hello World     # Test loading an extension by suffix
    : .name         cool example    # Test defining a property
    ```

    ```runmd .. alt
    + .test test/test.node-2        # Test adding a different block
    ```
    ",
    );

    while let Some(t) = lex.next() {
        println!("{:?} -- '{}'", t, lex.slice().trim());
    }
    println!("{:#?}", lex.extras);
}
