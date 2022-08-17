use logos::{Logos, Lexer};


#[derive(Logos)]
pub enum BlockIdentity {
    /// Symbol text, this is either name or symbol name
    ///
    #[regex("[A-Za-z]+[A-Za-z-._:0-9]*", on_symbol)]
    Symbol(String),
    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

fn on_symbol(lexer: &mut Lexer<BlockIdentity>) -> Option<String> {
    let mut slice = lexer.slice();
    if slice.starts_with('#') {
        slice = &slice[1..];
    }

    Some(slice.to_string())
}
