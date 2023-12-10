use logos::{Filter, Lexer, Logos};

use super::{
    prelude::{Context, Input},
    LexerExtensions,
};

/// Read a single property,
///
pub struct ReadProp;

impl ReadProp {
    /// Parses a simple name property string,
    ///
    pub fn parse(self, input: &str) -> Option<(String, String)> {
        let mut lexer = PropReader::lexer(input);
        match lexer.next() {
            Some(next) => match next {
                Ok(prop) => match prop {
                    PropReader::Json(p) | PropReader::Toml(p) => {
                        Some((p.0.to_string(), p.1.input_str()))
                    }
                },
                Err(_) => None,
            },
            None => None,
        }
    }
}

/// Special utility lexer,
///
/// Detect either `json` or `toml`. Emit (Key, Value).
///
#[derive(Logos, Clone)]
#[logos(skip r"[\r\n ]")]
#[logos(extras = Context<'s>)]
enum PropReader<'s> {
    #[regex(r#"["][a-zA-Z]*[a-zA-Z0-9]+["][ ]*:"#, on_json)]
    #[regex(r#"[a-zA-Z]*[a-zA-Z0-9]+[ ]*:"#, on_json)]
    Json(Prop<'s>),
    #[regex(r#"[a-zA-Z]*[a-zA-Z0-9]+[ ]*="#, on_toml)]
    Toml(Prop<'s>),
}

/// Property key-value-pair,
///
#[derive(Clone)]
struct Prop<'a>(pub &'a str, pub Input<'a>);

fn on_json<'a>(lex: &mut Lexer<'a, PropReader<'a>>) -> Filter<Prop<'a>> {
    let name = lex
        .slice()
        .trim_matches(|c| c == '"' || c == ':' || c == '[' || c == ']')
        .trim();
    let mut input_lexer: Lexer<Input> = lex.clone().morph();

    if let Some(Ok(input)) = input_lexer.next() {
        *lex = input_lexer.morph();
        lex.bump_line();
        Filter::Emit(Prop(name, input))
    } else {
        Filter::Skip
    }
}

fn on_toml<'a>(lex: &mut Lexer<'a, PropReader<'a>>) -> Filter<Prop<'a>> {
    let name = lex
        .slice()
        .trim()
        .trim_matches(|c| c == '=' || c == '[' || c == ']')
        .trim();
    let mut input_lexer: Lexer<Input> = lex.clone().morph();

    if let Some(Ok(input)) = input_lexer.next() {
        *lex = input_lexer.morph();
        Filter::Emit(Prop(name, input))
    } else {
        Filter::Skip
    }
}

#[test]
fn test_prop_reader() {
    let (name, value) = ReadProp.parse("input = hello world").unwrap();
    assert_eq!("input", name.as_str());
    assert_eq!("hello world", value);
}
