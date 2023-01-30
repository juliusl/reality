mod extension;
use std::collections::HashMap;

pub use extension::Extension;

mod root;
pub use root::Root;
use specs::World;
use tracing::{event, Level, trace};

use crate::{Keywords, Parser};

/// Enumeration of compile errors,
///
pub enum CompileError {}

/// Struct for compilation of .runmd
///
/// With this type it is possible to split up compile-time data mangling and runtime execution into different worlds,
///
/// This means that settings for execution can be compiled w/ components in one world, and runtime state can live in another,
///
pub struct Compiler {
    /// Main runmd parser,
    ///
    parser: Option<Parser>,
    /// If an extension is called that references a sub-world, then a new world is generated and added here,
    ///
    world_map: HashMap<String, World>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            parser: Some(Parser::new()),
            world_map: HashMap::default(),
        }
    }

    pub fn new_with(parser: Parser) -> Self {
        Self {
            parser: Some(parser),
            world_map: HashMap::default(),
        }
    }

    pub fn with_root(self, name: impl Into<String>) -> Self {
        self
    }

    pub fn with_extension(self, name: impl Into<String>) -> Self {
        self
    }

    /// Compile runmd content,
    ///
    pub fn compile(&mut self, runmd: impl Into<String>, symbol: Option<impl Into<String>>) {
        if let Some(mut parser) = self.parser.take() {
            let runmd = runmd.into();

            if let Some(symbol) = symbol.map(|s| s.into()) {
                parser.set_implicit_symbol(symbol);
            } else {
                parser.unset_implicit_symbol();
            }

            if let Some(mut token) = parser.parse_once(runmd) {
                if let (Some(entity), name, symbol, value) = token.line_info() {
                    trace!("{:?} {:?} {:?} {:?} {:?}", token.keyword(), entity, name, symbol, value);
                }

                while let Some(_token) = token.parse_next() {
                    if let (Some(entity), name, symbol, value) = _token.line_info() {
                        trace!("{:?} {:?} {:?} {:?} {:?}", _token.keyword(), entity, name, symbol, value);
                    }

                    token = _token;
                }
            }
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use specs::WorldExt;
    use tracing::trace;
    use crate::{Parser, CustomAttribute, Value};
    use super::Compiler;

    #[test]
    #[tracing_test::traced_test]
    fn test() {
        let mut parser = Parser::new();
        parser.add_custom_attribute(CustomAttribute::new_with("test_root", |a, c| {
            let entity = a.world().unwrap().entities().create();
            a.define_child(entity, "test_root", Value::Symbol(c));

            a.add_custom_with("load", |a, c| {
                let last = a.last_child_entity().unwrap();

                a.define_child(last, "load", Value::Symbol(c));
            });
        }));

        let mut compiler = Compiler::new_with(parser);
        compiler.compile(
            r#"
        ``` start
        + .test_root root_name
        : .load name_1_test
        : .load name_2_test
        ```
        "#.trim(),
            Some("test"),
        );

    //    let index =  compiler.parser.unwrap().get_block("test", "start").index();
    //    trace!("{:?}", index);
    }
}
