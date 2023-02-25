use specs::Builder;
use specs::Join;
use specs::World;
use specs::WorldExt;
use specs::Write;
use specs::WriteStorage;
use std::path::Path;
use std::sync::Arc;
use tracing::trace;

use self::interop::MakePacket;
use crate::parser::PropertyAttribute;
use crate::CustomAttribute;
use crate::Error;
use crate::Identifier;
use crate::Metadata;
use crate::Source;

mod interop;
pub use interop::Packet;
pub use interop::PacketHandler;

/// V2 runmd parser,
///
#[derive(Default)]
pub struct Parser {
    /// V1 Parser,
    ///
    /// The V1 parser will not
    ///
    v1_parser: Option<crate::Parser>,
    /// Caches a packet,
    ///
    packet_cache: Option<Packet>,
    /// Current root identifier,
    ///
    /// If set, will be used as the parent identifier for incoming define packets
    ///
    root_identifier: Option<Arc<Identifier>>,
    /// Current block identifier,
    ///
    block_identifier: Identifier,
}

impl Parser {
    /// Returns a new parser,
    ///
    pub fn new() -> Self {
        let v1_parser = Some(Self::new_v1_parser(None));
        Parser {
            v1_parser,
            packet_cache: None,
            root_identifier: None,
            block_identifier: Default::default(),
        }
    }

    /// Parses a file,
    ///
    pub fn parse_file(
        mut self,
        path: impl AsRef<Path>,
        packet_handler: &mut impl PacketHandler,
    ) -> Result<Self, Error> {
        if let Some(v1_parser) = self.v1_parser.as_mut() {
            let path = path.as_ref().canonicalize()?;

            let content = std::fs::read_to_string(&path)?;

            if let Some(family_name) = path
                .file_name()
                .and_then(|f| f.to_str())
                .and_then(|f| f.split_once("."))
                .map(|(prefix, _)| prefix)
                .filter(|p| !p.is_empty())
            {
                trace!("Setting implicit family name {family_name}");
                v1_parser.set_implicit_symbol(family_name);
            }

            v1_parser.set_metadata(Metadata::new(Source::file(path)));

            self.parse(content, packet_handler)
        } else {
            Err("Unitialized parser".into())
        }
    }

    /// Parses content and routes interop packets,
    ///
    /// Returns self w/ a v1_parser and updated World if successful,
    ///
    pub fn parse(
        mut self,
        content: impl Into<String>,
        packet_handler: &mut impl PacketHandler,
    ) -> Result<Self, Error> {
        if let Some(parser) = self.v1_parser.take() {
            // V1 Parser will emit packets
            let parser = parser.parse(content.into());
            let mut world = parser.commit();
            world.maintain();

            // Ensure an empty parser is added
            world.insert(Self::default());
            world.exec(|(mut p, mut parser): (WriteStorage<Packet>, Write<Self>)| {
                for mut _p in p.drain().join() {
                    parser.route_packet(_p, packet_handler)?;
                }

                parser.route_cache(packet_handler)?;
                Ok::<(), Error>(())
            })?;

            // Ensure existing parser is removed
            if let Some(existing) = world.remove::<Self>() {
                debug_assert!(
                    existing.packet_cache.is_none(),
                    "Packet cache should be empty"
                );
            }

            // Update v1_parser
            self.v1_parser = Some(Self::new_v1_parser(Some(world)));
            Ok(self)
        } else {
            Err("Trying to parse w/ an unintialized parser".into())
        }
    }

    /// Routes an incoming packet,
    ///
    /// If the packet is an extension packet, it will be cached so that subsequent packets can be merged if applicable,
    ///
    /// Otherwise, the packet is routed to the destination packet handler.
    ///
    /// When a packet cannot be merged, the cache is cleared and routed to the destination handler.
    ///
    fn route_packet(
        &mut self,
        mut incoming: Packet,
        dest: &mut impl PacketHandler,
    ) -> Result<(), Error> {
        // Keep track of current block_identifier
        if self.block_identifier != incoming.block_identifier {
            self.block_identifier = incoming.block_identifier.clone();
            self.root_identifier.take();
        }

        if let Some(root_id) = self.root_identifier.as_ref().filter(|_| !incoming.is_add()) {
            incoming.identifier.set_parent(root_id.clone());
        }

        if let Some(cached) = self.packet_cache.as_mut() {
            // Merge errors signal that the cache should be routed,
            // The rejected packet will be re-routed
            if let Err(err) = cached.merge_packet(incoming) {
                match err {
                    interop::MergeRejectedReason::DifferentBlockNamespace(next)
                    | interop::MergeRejectedReason::NewRoot(next)
                    | interop::MergeRejectedReason::NewExtension(next) => {
                        self.route_cache(dest)?;
                        self.route_packet(next, dest)?;
                    }
                    interop::MergeRejectedReason::UnrelatedPacket(unrelated) => {
                        unreachable!("If a packet is rejected for this reason it indicates a packet creation error, {:?}", unrelated)
                    }
                }
            }

            Ok(())
        } else if incoming.is_extension() {
            trace!("Packet cache is being updated, {:#}", incoming.identifier);
            // Hold on to incoming extension packets
            self.packet_cache = Some(incoming);
            Ok(())
        } else {
            if incoming.is_add() {
                let block = Arc::new(incoming.block_identifier.clone());
                incoming.identifier.set_parent(block.clone());
                let mut next_root = incoming.identifier.clone();
                next_root.set_parent(block);
                self.root_identifier = Some(Arc::new(next_root));
            }

            dest.on_packet(incoming)
        }
    }

    /// Clears and routes the cached packet to the destination packet handler,
    ///
    fn route_cache(&mut self, dest: &mut impl PacketHandler) -> Result<(), Error> {
        if let Some(last) = self.packet_cache.take() {
            trace!(
                "Cached packet is being routed to dest, {:#}",
                last.identifier
            );
            dest.on_packet(last)?;
        }

        Ok(())
    }

    /// Returns a new v1 parser that will emit interop packets on custom attributes,
    ///
    fn new_v1_parser(world: Option<World>) -> crate::Parser {
        let mut v1_parser = world
            .map(|w| crate::Parser::new_with(w))
            .unwrap_or(crate::Parser::new());

        v1_parser.set_default_custom_attribute(CustomAttribute::new_with("", |parser, _| {
            if let Ok(packet) = parser.try_make_packet() {
                parser.lazy_exec_mut(move |w| {
                    w.register::<Packet>();
                    w.create_entity().with(packet).build();
                });
            }
        }));

        v1_parser.set_default_property_attribute(PropertyAttribute(|parser| {
            if let Ok(packet) = parser.try_make_packet() {
                parser.lazy_exec_mut(move |w| {
                    w.register::<Packet>();
                    w.create_entity().with(packet).build();
                });
            }
        }));
        v1_parser
    }
}

#[allow(unused_imports)]
mod tests {
    use super::Parser;
    use crate::{
        state::Provider,
        v2::{
            compiler::Compiled,
            data::{query::Query, toml::TomlProperties},
            property_value,
            thunk::Update,
            thunk_call,
            toml::DocumentBuilder,
            BlockList, Call, Compiler, Object, Properties,
        },
        BlockProperties, Error, Identifier,
    };
    use async_trait::async_trait;
    use serde::Deserialize;
    use specs::{Join, ReadStorage, WorldExt};
    use toml_edit::Document;
    use tracing::trace;
    use tracing_test::traced_test;

    #[tokio::test]
    // #[traced_test]
    async fn test_parser() {
        let runmd = r#"
``` b
: test .true

+ test:v1 .op add
: lhs .int
: rhs .int
: sum .int
<> .input lhs : .type stdin
<test> .input rhs
<> .eval  sum

+ .op add
: lhs .float
: rhs .float
: sum .float
<> .input lhs : .type stdin
<> .input rhs : .type stdin
<> .eval  sum
```

``` a
+ test:v2 .op sub
: lhs .int
: rhs .int
: diff .int
<> .input lhs : .type stdin
: count .int 1
: test .env /host
: rust_log .env reality=trace
: first  .arg hello
: second .arg world
<test> .input rhs
<> debug .eval diff

+ .op mult
: lhs .int
: rhs .int
: prod .int
<> .input lhs
<> .input rhs
<> .eval prod
```

``` host
: RUST_LOG  .env reality=trace
: HOST      .env test.io 

+ .host
: RUST_LOG  .env reality=trace
: HOST      .env test.io 
```

```
: root_test .true

+ .host
: RUST_LOG  .env reality=trace
: HOST      .env test.io 
```
"#;

        std::fs::create_dir_all(".test").expect("should be able to create test dir");
        std::fs::write(".test/test.runmd", runmd).expect("should be able to write");

        let mut compiler = Compiler::new();
        let parser = Parser::new();
        let _parser = parser.parse_file(".test/test.runmd", &mut compiler);
        let _ = compiler.compile().expect("should be able to build self");

        let mut doc_builder = DocumentBuilder::new();
        let mut build_properties = Properties::new(Identifier::default());
        compiler.update_last_build(&mut doc_builder).map(|l| {
            if let Some(toml) = compiler.as_ref().read_component::<TomlProperties>().get(l) {
                println!("{}", toml.doc);

                toml["properties"].as_table().map(|t| {
                    for (k, _) in t.iter() {
                        println!("properties - {k}");
                    }
                });

                toml["block"].as_table().map(|t| {
                    for (k, _) in t.iter() {
                        println!("block - {k}");
                    }
                });

                toml["root"].as_table().map(|t| {
                    for (k, _) in t.iter() {
                        println!("root - {k}");
                    }
                });
            }
        });

        compiler.update_last_build(&mut build_properties).map(|l| {
            if let Some(prop) = compiler.as_ref().read_component::<Properties>().get(l) {
                println!("{:#?}", prop);
            }
        });

        compiler.last_build_log().map(|b| {
            for (_, e) in b.index() {
                compiler
                    .compiled()
                    .state::<Object>(*e)
                    .map(|o| {
                        let mut properties = o.properties().clone();
                        properties["testing_update"] = property_value(true);
                        properties
                    })
                    .map(|update| {
                        compiler.compiled().update(*e, &update).ok();
                    });
            }

            b.search("input.(var)")
                .for_each(|(ident, interpolated, entity)| {
                    trace!("Installing thunk for input extension -- {:#}", ident);
                    if let Some(var) = interpolated.get("var").cloned() {
                        compiler
                            .as_ref()
                            .write_component()
                            .insert(*entity, thunk_call(TestInput(var)))
                            .ok();
                    }
                })
        });
        compiler.as_mut().maintain();

        let mut test = DocumentBuilder::new();
        compiler.visit_last_build(&mut test);

        let doc: TomlProperties = (&test).into();
        println!("{}", doc.doc);

        let runtime = tokio::runtime::Runtime::new().unwrap();
        compiler.as_mut().insert(Some(runtime.handle().clone()));

        if let Some(b) = compiler.last_build() {
            let mut joinset = compiler
                .compiled()
                .batch_call_matches(*b, "input.(var)")
                .expect("should return a set");

            while let Some(Ok(result)) = joinset.join_next().await {
                if let Ok((ident, interpolated, prop)) = result {
                    if let Some(var) = interpolated.get("var").cloned() {
                        trace!("{:#} -- {:?}", ident, prop);
                        assert_eq!(Some(100), prop[&var].as_int());
                    }
                }
            }
        }

        let test_root = "test.b.block.op.add.test:v1".parse::<Identifier>().unwrap();
        let test_root = doc
            .deserialize::<TestRoot>(&test_root)
            .expect("should deserialize");
        println!("{:?}", test_root);

        let test_root = "test.a.block.op.sub.test:v2.input.lhs"
            .parse::<Identifier>()
            .unwrap();
        let test_root = doc
            .deserialize_keys::<TestEnv>(&test_root, "env")
            .expect("should deserialize");
        println!("{:?}", test_root);

        for (ident, _, props) in compiler
            .compiled()
            .query("input.(var)", |_, map, props| {
                map["var"] == "lhs"
                    && props["type"]
                        .as_symbol()
                        .filter(|s| *s == "stdin")
                        .is_some()
                    && props["rust_log"].as_symbol().is_some()
            })
            .unwrap()
        {
            println!("{}", ident);
            println!("{:?}", props);
        }

        for (ident, _, props) in doc
            .query(r#"test:v1.test.input.(var)"#, |_, _, _| true)
            .unwrap()
        {
            println!("{} {:?}", ident, props);
        }

        runtime.shutdown_background();
    }

    struct TestInput(String);

    #[async_trait]
    impl Call for TestInput {
        async fn call(&self) -> Result<Properties, Error> {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            let mut result = Properties::default();
            result[&self.0] = property_value(100);

            Ok(result)
        }
    }

    #[allow(dead_code)]
    #[derive(Deserialize, Debug)]
    struct TestRoot {
        lhs: i64,
        rhs: i64,
        sum: i64,
    }

    #[allow(dead_code)]
    #[derive(Deserialize, Debug)]
    struct TestEnv {
        test: String,
        rust_log: String,
    }
}
