use atlier::system::Value;
use logos::{Lexer, Logos};

use crate::parser::Elements;
use crate::Parser;

/// Parser keywords and symbols,
///
#[derive(Logos, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[logos(extras = Parser)]
pub enum Keywords {
    /// Write a stable attribute
    ///
    #[token("add", on_add)]
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
    Comment = 0x0C,

    /// Writes a transient attribute
    ///
    /// If `::` is used, the current attribute parser will be reused.
    ///
    #[token("define", on_define)]
    #[token(":", on_define)]
    Define = 0x0D,

    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error = 0xFF,
}

impl From<u8> for Keywords {
    fn from(char: u8) -> Self {
        match char {
            0x0A => Keywords::Add,
            0x0B => Keywords::BlockDelimitter,
            0x0C => Keywords::Comment,
            0x0D => Keywords::Define,
            _ => Keywords::Error,
        }
    }
}

fn on_comment(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        lexer.bump(next_line.len());
    }
}

fn on_block_delimitter(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        let mut block_ident = Elements::lexer(next_line);

        match (block_ident.next(), block_ident.next()) {
            (Some(Elements::Identifier(name)), Some(Elements::Identifier(symbol))) => {
                let current = lexer.extras.lookup_block(name, symbol);
                lexer.extras.parsing = Some(current);
            }
            (Some(Elements::Identifier(symbol)), _) => {
                lexer.extras.evaluate_stack();
                let name = lexer.extras.current_block().name().to_string();
                let current = lexer.extras.lookup_block(name, symbol);
                lexer.extras.parsing = Some(current);
            }
            // Only enable this new behavior if implicit_block_symbol is enabled
            (None, None) if lexer.extras.implicit_block_symbol.is_some() => {
                lexer.extras.evaluate_stack();
                let current = lexer.extras.lookup_block("", "");
                lexer.extras.parsing = Some(current);
            }
            _ => {
                lexer.extras.evaluate_stack();
                // If an ident is not set, then
                lexer.extras.parsing = None;
            }
        }
        lexer.bump(next_line.len());
    }
}

fn on_add(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        lexer.extras.new_attribute().parse(next_line.trim());

        lexer.bump(next_line.len());
    }
}

fn on_define(lexer: &mut Lexer<Keywords>) {
    if let Some(next_line) = lexer.remainder().lines().next() {
        // Syntax sugar for,
        // From -
        // add connection .empty
        // define connection host .text example.com
        // Sugar -
        // add connection .empty
        // :: host .text example.com
        //
        if lexer.slice().starts_with(":") {
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

            // Because this is a property, set the value to empty
            attr_parser.set_value(Value::Empty);
            attr_parser.parse(next_line.trim());
        } else {
            // In keyword form, the expectation is that name/symbol will be present
            let attr_parser = lexer.extras.new_attribute();
            attr_parser.parse(next_line.trim());
        }

        lexer.bump(next_line.len());
    }
}
