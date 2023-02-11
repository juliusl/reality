mod extension;
use std::collections::HashMap;

pub use extension::Extension;
pub use extension::ExtensionCompileFunc;

mod root;
pub use root::Root;

mod extension_parser;
pub use extension_parser::ExtensionParser;


use specs::Join;
use specs::LazyUpdate;
use specs::ReadStorage;
use specs::WorldExt;
use tracing::trace;

use crate::{parser::LineInfo, Keywords, Parser};

use self::extension::ExtensionThunk;

/// Enumeration of compiler errors,
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
    /// Collection of extensions supported by the compiler,
    ///
    /// Extensions can be added directly to the compiler, or through a root implementation
    ///
    extensions: HashMap<String, ExtensionThunk<Parser>>,
}

impl Compiler {
    pub fn new() -> Self {
        let parser = Parser::new();
        parser.world().read_resource::<LazyUpdate>().exec_mut(|world| world.register::<Root>());
        Self {
            parser: Some(parser),
            extensions: HashMap::default(),
        }
    }

    pub fn new_with(parser: Parser) -> Self {
        parser.world().read_resource::<LazyUpdate>().exec_mut(|world| world.register::<Root>());
        Self {
            parser: Some(parser),
            extensions: HashMap::default(),
        }
    }

    pub fn with_extension<E>(mut self) -> Self
    where
        E: Extension<Parser>,
    {
        self.extensions
            .insert(E::ident().to_string(), E::as_thunk());
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

            if let Ok(mut token) = parser.parse_once(runmd) {
                loop {
                    match token.parse_next() {
                        Ok(mut _token) => {
                            match _token.keyword() {
                                // If an extension was just parsed, we need to find the impl
                                // and install parsers -- 
                                Keywords::Extension => {
                                    if let LineInfo {
                                        name: Some(extension_name),
                                        ..
                                    } = _token.line_info()
                                    {
                                        if let Some(ExtensionThunk(parser, compile)) =
                                            self.extensions.get(extension_name)
                                        {
                                            parser(_token.parser_mut());
                                            _token.add_compile(*compile);
                                        }
                                    }
                                }
                                _ => {}
                            }

                            if let Keywords::Error = _token.keyword() {

                            } else {
                                let LineInfo {
                                    name,
                                    entity,
                                    symbol,
                                    value,
                                    ..
                                } = _token.line_info();
                                trace!(
                                    "{:?} {:?} {:?} {:?} {:?}",
                                    _token.keyword(),
                                    entity,
                                    name,
                                    symbol,
                                    value
                                );
                            }

                            token = _token;
                        }
                        Err(parser) => {
                            self.parser = Some(parser);
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn link(mut self) {
        if let Some(parser) = self.parser.take() {
            let mut world = parser.commit();
            world.maintain();

            world.exec(|roots: ReadStorage<Root>| {
                for root in roots.join() {
                    // TODO:
                    trace!("Found root --- {}", root.ident());
                }
            });
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use super::{Compiler, Extension};
    use crate::{compiler::ExtensionParser, AttributeParser, CustomAttribute, Parser, Value};
    use specs::WorldExt;
    use toml_edit::{Document, table, value, array, Item, ArrayOfTables, Table};
    use tracing::trace;

    struct TestExtension;

    impl<Parser: ExtensionParser> Extension<Parser> for TestExtension {
        fn ident() -> &'static str {
            "test_extension"
        }

        fn parser(extension_parser: &mut Parser) {
            extension_parser.parse_symbol("load");
            extension_parser.parse_number("test_number");
            extension_parser.parse_bool("test_bool");
        }

        fn compile(
            _: specs::EntityBuilder,
            _: crate::BlockProperties,
        ) -> Result<specs::Entity, super::CompileError> {
            todo!()
        }
    }

    #[test]
    fn test_toml_doc() {
        let mut doc = Document::new();

        doc["."] = table();
        doc["."]["roots"] = Item::ArrayOfTables(ArrayOfTables::new());
        doc["."]["roots"].as_array_of_tables_mut().map(|t| {
            t.push(Table::new());
            t.push(Table::new());
            t.push(Table::new());
        });

        doc["."]["roots"][0]["tag"] = value("");
        doc["."]["roots"][0]["symbol"] = value("operation");
        doc["."]["roots"][0]["name"] = value("resolve.manifest");
        doc["."]["operation/resolve.manifest"] = Item::ArrayOfTables(ArrayOfTables::new());
        doc["."]["operation/resolve.manifest"].as_array_of_tables_mut().map(|t| {
            t.push(Table::new());
            t.push(Table::new());
        });
        
        doc["."]["operation/resolve.manifest"][0] = table();
        doc["."]["operation/resolve.manifest"][0]["println"] = value("message");

        doc["."]["operation/resolve.manifest"][1] = table();
        doc["."]["operation/resolve.manifest"][1]["println"] = value("message2");


        doc["."]["roots"][1]["tag"] = value("");
        doc["."]["roots"][1]["symbol"] = value("operation");
        doc["."]["roots"][1]["name"] = value("pull.blob");
        doc["."]["operation/pull.blob"] = Item::ArrayOfTables(ArrayOfTables::new());
        doc["."]["operation/pull.blob"].as_array_of_tables_mut().map(|t| {
            t.push(Table::new());
            t.push(Table::new());
        });
        
        doc["."]["operation/pull.blob"][0] = table();
        doc["."]["operation/pull.blob"][0]["println"] = value("message");

        doc["."]["operation/pull.blob"][1] = table();
        doc["."]["operation/pull.blob"][1]["println"] = value("message2");

        doc["."]["roots"][2]["tag"] = value("debug");
        doc["."]["roots"][2]["symbol"] = value("operation");
        doc["."]["roots"][2]["name"] = value("pull.blob");
        doc["."]["operation/pull.blob#debug"] = Item::ArrayOfTables(ArrayOfTables::new());
        doc["."]["operation/pull.blob#debug"].as_array_of_tables_mut().map(|t| {
            t.push(Table::new());
            t.push(Table::new());
        });
        
        doc["."]["operation/pull.blob#debug"][0] = table();
        doc["."]["operation/pull.blob#debug"][0]["println"] = value("message");

        doc["."]["operation/pull.blob#debug"][1] = table();
        doc["."]["operation/pull.blob#debug"][1]["println"] = value("message2");
        println!("{}", doc);
    }

    #[test]
    #[tracing_test::traced_test]
    fn test() {
        let mut parser = Parser::new();
        parser.add_custom_attribute(CustomAttribute::new_with("test_root", |a, c| {
            let entity = a.world().unwrap().entities().create();
            a.define_child(entity, "test_root", Value::Symbol(c));
            a.set_id(entity.id());
        }));

        let mut compiler = Compiler::new_with(parser)
            .with_extension::<TestExtension>();

        compiler.compile(
            r#"
        ``` start
        + .test_root root_name
        <> test_extension : .load name_1_test : .load name_2_test : .test_number .float 0.10 : load .test_number .float 0.12 : .test_bool
        ```
        "#
            .trim(),
            Some("test"),
        );

        compiler.link();

        //    let index =  compiler.parser.unwrap().get_block("test", "start").index();
        //    trace!("{:?}", index);
    }
}
