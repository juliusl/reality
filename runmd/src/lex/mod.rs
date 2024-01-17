mod attribute;
mod block;
mod context;
mod extension;
mod input;
mod instruction;
mod line;
mod prop;
mod tag;
mod tokens;

pub mod prelude {
    pub use super::attribute::Attribute;
    pub use super::block::Block;
    pub use super::context::Context;
    pub use super::extension::Extension;
    pub use super::input::Input;
    pub use super::instruction::Instruction;
    pub use super::line::Line;
    pub use super::prop::ReadProp;
    pub use super::tag::Tag;
    pub use super::tokens::Tokens;

    pub use logos::Filter;
    pub use logos::Lexer;
    pub use logos::Logos;

    pub use super::LexerExtensions;
}

/// Extension functions for working with the Lexer,
///
pub trait LexerExtensions<'a> {
    /// Bump the lexer to the next terminator,
    ///
    fn bump_terminator(&mut self, pat: &[char]) -> Option<&'a str>;

    /// Bump the lexer to the next line,
    ///
    fn bump_line(&mut self) -> Option<&'a str>;
}

impl<'a, T> LexerExtensions<'a> for logos::Lexer<'a, T>
where
    T: logos::Logos<'a, Source = str>,
{
    fn bump_line(&mut self) -> Option<&'a str> {
        if let Some(next) = self.remainder().lines().next() {
            self.bump(next.len() + 1);
            Some(next)
        } else {
            None
        }
    }

    fn bump_terminator(&mut self, pat: &[char]) -> Option<&'a str> {
        if let Some(next) = self.remainder().split_terminator(pat).next() {
            self.bump(next.len() + 1);
            Some(next)
        } else {
            None
        }
    }
}

#[test]
fn test_lexer_extensions() {
    use logos::Logos;

    #[derive(Logos)]
    enum Test {
        #[token("A")]
        A,
        #[token("B")]
        B,
        #[token("C")]
        C,
    }

    let mut lex = Test::lexer(
        r"
    Lorem Ipsum is simply dummy text of the printing and typesetting industry.
    Lorem Ipsum has been the industry's standard dummy text ever since the 1500s, when an unknown printer took a galley of type and scrambled it to make a type specimen book. 
    It has survived not only five centuries, but also the leap into electronic typesetting, remaining essentially unchanged. 
    It was popularised in the 1960s with the release of Letraset sheets containing Lorem Ipsum passages, and more recently with desktop publishing software 
    like Aldus PageMaker including versions of Lorem Ipsum.
    ",
    );

    lex.bump_line();
    assert_eq!(
        lex.remainder(),
        r"    Lorem Ipsum is simply dummy text of the printing and typesetting industry.
    Lorem Ipsum has been the industry's standard dummy text ever since the 1500s, when an unknown printer took a galley of type and scrambled it to make a type specimen book. 
    It has survived not only five centuries, but also the leap into electronic typesetting, remaining essentially unchanged. 
    It was popularised in the 1960s with the release of Letraset sheets containing Lorem Ipsum passages, and more recently with desktop publishing software 
    like Aldus PageMaker including versions of Lorem Ipsum.
    "
    );

    lex.bump_terminator(&[',']);
    assert_eq!(
        lex.remainder(),
        r" when an unknown printer took a galley of type and scrambled it to make a type specimen book. 
    It has survived not only five centuries, but also the leap into electronic typesetting, remaining essentially unchanged. 
    It was popularised in the 1960s with the release of Letraset sheets containing Lorem Ipsum passages, and more recently with desktop publishing software 
    like Aldus PageMaker including versions of Lorem Ipsum.
    "
    );
}
