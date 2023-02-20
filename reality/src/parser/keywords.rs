use logos::{Lexer, Logos, Filter};
use crate::parser::Elements;
use crate::{Parser, Identifier};

/// Parser keywords and symbols,
///
#[derive(Logos, Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
#[logos(extras = Parser)]
pub enum Keywords {
    /// Write a stable attribute
    ///
    #[token("+", on_add)]
    Add = 0x0A,

    /// Block delimitter either starts or ends a block
    ///
    /// If starting a block, the delimitter can be followed by two
    /// symbols representing the block name and block symbol.
    ///
    /// If this is the start of a block, with no name/symbol, then
    /// the root block will be used as the context.
    ///
    #[token("```", on_block_delimitter)]
    #[token("<```", on_block_delimitter)]
    #[token("<```>", on_block_delimitter)]
    BlockDelimitter = 0x0B,

    /// Comments are skipped, usually .md list element or header so that the .runmd can be
    /// partially cross compatible w/ .md.
    ///
    #[token("#", on_comment)]
    #[token("*", on_comment)]
    #[token("-", on_comment)]
    #[token("//", on_comment)]
    #[token("``` md", on_comment)]
    #[token("``` runmd", on_comment)]
    #[token("```md", on_comment)]
    #[token("```runmd", on_comment)]
    #[token("<", on_inline_comment)]
    Comment = 0x0C,

    /// Writes a transient attribute
    ///
    /// If `::` is used, the current attribute parser will be reused.
    ///
    #[token(":", on_define)]
    Define = 0x0D,

    /// Extension keyword, allows for wire protocol to include user frames
    ///
    #[regex(r"<[a-zA-Z0-9]+>", on_extension)]
    #[token("<>", on_extension)]
    Extension = 0x0E,

    /// New line
    /// 
    #[token("\r")]
    #[token("\n")]
    #[token("\r\n")]
    NewLine = 0xF0,
    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\fa-zA-Z.]+", logos::skip)]
    Error = 0xFF,
}

impl From<u8> for Keywords {
    fn from(char: u8) -> Self {
        match char {
            0x0A => Keywords::Add,
            0x0B => Keywords::BlockDelimitter,
            0x0C => Keywords::Comment,
            0x0D => Keywords::Define,
            0x0E => Keywords::Extension,
            _ => Keywords::Error,
        }
    }
}

fn on_comment(lexer: &mut Lexer<Keywords>) -> Filter<()> {
    if !lexer.extras.enabled {
        return Filter::Skip;
    }
    
    if let Some(next_line) = lexer.remainder().lines().next() {
        lexer.bump(next_line.len());
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

fn on_inline_comment(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        lexer.bump(next_line.find(">").unwrap_or(next_line.len()));
    }
}

fn on_block_delimitter(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        let mut block_ident = Elements::lexer(next_line);

        match (block_ident.next(), block_ident.next()) {
            (Some(Elements::Identifier(name)), Some(Elements::Identifier(symbol))) => {
                let current = lexer.extras.ensure_block(name, symbol);
                lexer.extras.parsing = Some(current);
                lexer.extras.enabled = true;
            }
            (Some(Elements::Identifier(symbol)), _) => {
                lexer.extras.evaluate_stack();
                let name = lexer.extras.current_block().name().to_string();
                let current = lexer.extras.ensure_block(name, symbol);
                lexer.extras.parsing = Some(current);
                lexer.extras.enabled = true;
            }
            // Only enable this new behavior if implicit_block_symbol is enabled
            (None, None) if lexer.extras.implicit_block_symbol.is_some() => {
                lexer.extras.evaluate_stack();
                let current = lexer.extras.ensure_block("", "");
                lexer.extras.parsing = Some(current);
                lexer.extras.enabled = true;
            }
            _ => {
                lexer.extras.evaluate_stack();
                // If an ident is not set, then
                lexer.extras.parsing = None;
                lexer.extras.enabled = !lexer.extras.enabled;
            }
        }
        lexer.bump(next_line.len());
    }
}

fn on_add(lexer: &mut Lexer<Keywords>) -> Filter<()> {
    if !lexer.extras.enabled {
        return Filter::Skip;
    }

    if let Some(next_line) = lexer.remainder().lines().next() {
        let bump = {
            lexer
                .extras
                .new_attribute()
                .set_keyword(Keywords::Add)
                .parse(next_line)
                .last_parse_len()
        };

        let bump = bump;
        lexer.bump(bump);
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}

fn on_define(lexer: &mut Lexer<Keywords>) -> Filter<()> {
    if !lexer.extras.enabled {
        return Filter::Skip;
    }

    let input = lexer.remainder();

    let bump = if lexer.slice().starts_with(":") {
        let current_block_symbol = lexer.extras.current_block_symbol().to_string();
        let attr_parser = lexer.extras.parse_property();

        if attr_parser.name().is_none() {
            if !current_block_symbol.is_empty() {
                attr_parser.set_name(current_block_symbol);
            } else {
                // todo
                panic!("Invalid syntax,\n{}", lexer.remainder())
            }
        }

        attr_parser.set_keyword(Keywords::Define).parse(input);
        attr_parser.last_parse_len()
    } else {
        // In keyword form, the expectation is that name/symbol will be present
        let attr_parser = lexer.extras.new_attribute();
        attr_parser.set_keyword(Keywords::Define).parse(input);
        attr_parser.last_parse_len()
    };

    lexer.bump(bump);

    Filter::Emit(())
}

fn on_extension(lexer: &mut Lexer<Keywords>) -> Filter<()> {
    if !lexer.extras.enabled {
        return Filter::Skip;
    }

    // Set a new extension symbol in the parser,
    if lexer.slice().len() > 2 {
        let extension_namespace = lexer.slice()[1..lexer.slice().len() - 1].to_string();
        lexer.extras.parse_property().set_implicit_identifier(Identifier::try_create_root(extension_namespace).ok().as_ref());
    } else {
        lexer.extras.parse_property().set_implicit_identifier(None);
    }
    
    if let Some(next_line) = lexer.remainder().lines().next() {
        let bump = {
            let attr_parser = lexer.extras.parse_property();
            attr_parser
                .set_keyword(Keywords::Extension)
                .parse(next_line)
                .last_parse_len()
        };

        let bump = bump;
        lexer.bump(bump);
        Filter::Emit(())
    } else {
        Filter::Skip
    }
}