use super::prelude::*;

/// Enumeration of Input variants to attributes or extension containers,
///
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(extras = Context<'s>)]
pub enum Input<'source> {
    /// Text value,
    ///
    #[regex(r##"[^\r\n'"`#]*"##, on_input)]
    Text(&'source str),
    /// Escaped text value,
    ///
    #[regex(r#"["][^"]*["]"#, on_escaped_input)]
    #[regex(r#"[`][^`]*[`]"#, on_escaped_input)]
    #[regex(r#"['][^']*[']"#, on_escaped_input)]
    #[regex(r#"["]["]["][.]*["]["]["]"#, on_escaped_input)]
    EscapedText(&'source str),
}

#[inline]
fn on_escaped_input<'s>(lex: &mut Lexer<'s, Input<'s>>) -> &'s str {
    lex.slice()
        .trim_matches(|c| c == '\'' || c == '\"' || c == '`')
}

#[inline]
fn on_input<'s>(lex: &mut Lexer<'s, Input<'s>>) -> Filter<&'s str> {
    if lex.slice().trim().is_empty() {
        // If the input is escaped, then any preceding spaces would be considered input
        Filter::Skip
    } else {
        Filter::Emit(lex.slice().trim())
    }
}

impl<'a> Input<'a> {
    /// Unwraps into the input str,
    /// 
    #[inline]
    pub fn input_str(self) -> &'a str {
        match self {
            Input::Text(s) | Input::EscapedText(s) => s,
        }
    }
}

#[test]
fn test_input_lexer() {
    let mut lex = Input::lexer(
        r"hello-world # Test comment",
    );
    assert_eq!(lex.next(), Some(Ok(Input::Text("hello-world"))));

    let mut lex = Input::lexer(
        r"  hello  world # Test comment",
    );
    assert_eq!(lex.next(), Some(Ok(Input::Text("hello  world"))));

    let mut lex = Input::lexer(
        r"   'hello-world'   # Test comment",
    );
    assert_eq!(lex.next(), Some(Ok(Input::EscapedText("hello-world"))));

    let mut lex = Input::lexer(
        r"   `hello-world`   # Test comment",
    );
    assert_eq!(lex.next(), Some(Ok(Input::EscapedText("hello-world"))));

    let mut lex = Input::lexer(
        r##"   "hello world"  # Test comment"##,
    );
    assert_eq!(lex.next(), Some(Ok(Input::EscapedText("hello world"))));
}
