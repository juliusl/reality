use specs::{
    Builder, Component, Entity, Join, ReadStorage, VecStorage, World, WorldExt, WriteStorage,
};

use crate::{CustomAttribute, Keywords};

use super::Action;
use super::Block;
use super::action;

mod interop;
pub use interop::Packet;
pub use interop::PacketHandler;

/// V2 block parser implementation,
///
pub struct Parser {
    /// V1 Parser,
    ///
    /// The V1 parser will not
    ///
    v1_parser: crate::Parser,
}

impl Parser {
    /// Returns a new parser,
    ///
    pub fn new() -> Self {
        let v1_parser = Self::new_v1_parser(None);
        Parser { v1_parser }
    }

    /// Parses content and emits interop packets,
    ///
    pub fn parse(
        mut self,
        content: impl Into<String>,
        packet_handler: &mut impl PacketHandler,
    ) -> Self {
        let parser = self.v1_parser.parse(content.into());

        let mut world = parser.commit();
        world.maintain();
        world.exec(
            |(mut p, blocks): (WriteStorage<Packet>, ReadStorage<crate::Block>)| {
                for mut _p in p.drain().join() {
                    if let Some(e) = _p.entity.and_then(|e| blocks.get(e)) {
                        _p.block_namespace = e.namespace();
                        
                        if let Some(Keywords::Add) = _p.keyword.as_ref() {
                            let map = e.map_transient(&_p.ident);
                            for (ident, value) in map.iter() {
                                _p.actions.push(action::with(ident, value.clone()));
                            }
                        }
                    }

                    if let Err(_) = packet_handler.on_packet(_p) {
                        todo!("error in parser");
                    }
                }
            },
        );

        self.v1_parser = Self::new_v1_parser(Some(world));
        self
    }

    /// Returns a new v1 parser that will emit interop packets on custom attributes,
    ///
    fn new_v1_parser(world: Option<World>) -> crate::Parser {
        let mut v1_parser = world
            .map(|w| crate::Parser::new_with(w))
            .unwrap_or(crate::Parser::new());

        v1_parser.set_default_custom_attribute(CustomAttribute::new_with("", |parser, input| {
            if let Some(ident) = parser.attr_ident().cloned() {
                let name = parser.name().cloned();
                let symbol = parser.symbol().cloned();
                let entity = parser.entity().clone();
                let keyword = parser.keyword().cloned();
                let extension_namespace = parser.extension_namespace();
                let line_count = parser.line_count();
                parser.lazy_exec_mut(move |w| {
                    w.register::<Packet>();
                    w.create_entity()
                        .with(Packet {
                            name,
                            entity,
                            symbol,
                            keyword,
                            ident,
                            input,
                            block_namespace: ".".to_string(),
                            extension_namespace: extension_namespace.unwrap_or_default(),
                            line_count,
                            actions: vec![]
                        })
                        .build();
                });
            }
        }));
        v1_parser
    }
}

#[allow(unused_imports)]
mod tests {
    use tracing_test::traced_test;

    use crate::v2::Compiler;

    use super::Parser;

    #[test]
    #[traced_test]
    fn test_parser() {
        let parser = Parser::new();
        let mut compiler = Compiler::default();
        // let parser = parser.parse(
        //     r#"
        // # ``` test block
        // # + test  .person         John
        // # :       .dob            10/10/1000
        // # :       .location       USA

        // # + .person John
        // # : test .dob 10/10/1000
        // # ```

  
        // "#,
        //     &mut compiler,
        // );

        let _parser = parser.parse(
            r#"
            ``` b block 
             + test .person Jacob
             : .dob 10/10/1000
            ```
        "#,
            &mut compiler,
        );

        for b in compiler.blocks() {
            println!("{:#?}", b);
            println!("{} {:?}", b.0, b.1.requires().collect::<Vec<_>>());
        }
    }
}
