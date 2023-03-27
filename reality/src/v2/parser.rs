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
            } else {
                v1_parser.unset_implicit_symbol();
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
    use std::{collections::BTreeMap, sync::Arc};

    use super::Parser;
    use crate::{
        state::Provider,
        v2::{
            command::{export_toml, import_toml},
            compiler::{BuildLog, Compiled, WorldWrapper},
            data::{
                query::{self, all, Predicate, Query},
                toml::TomlProperties,
            },
            properties::property_list,
            property_value,
            thunk::{auto::Auto, Update},
            thunk_call,
            toml::DocumentBuilder,
            BlockList, Call, Compiler, Interner, Object, Properties, ThunkCall, Visitor, Listener,
        },
        BlockProperties, Error, Identifier,
    };
    use async_trait::async_trait;
    use serde::Deserialize;
    use specs::{
        storage, Builder, Component, Entities, Join, LazyUpdate, Read, ReadStorage, RunNow, System,
        VecStorage, World, WorldExt, WriteStorage,
    };
    use tokio::{
        io::AsyncWriteExt,
        runtime::Handle,
        task::{JoinHandle, JoinSet},
    };
    use toml_edit::Document;
    use tracing::{trace, error};
    use tracing_test::traced_test;

    #[tokio::test]
    // #[traced_test]
    async fn test_parser() -> Result<(), Error> {
        let runmd = r#"
``` b
: test .true
: expr .int3 1, 2, 3

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
: test .bin aGVsbG8gd29ybGQ=
#: test .int 10
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

        // Test building state document
        // let mut doc_builder = DocumentBuilder::new();
        // let count_block_len_thunk = compiler
        //     .update_last_build(&mut doc_builder)
        //     // Map document builder into toml properties
        //     .map_into(|build| {
        //         let props: TomlProperties = build.into();
        //         Ok(props)
        //     })
        //     // Map toml properties to a call thunk
        //     .map_into(|toml| {
        //         // Test preparing the call
        //         let len = toml["block"]
        //             .as_table()
        //             .map(|t| t.len())
        //             .unwrap_or_default();
        //         Ok(thunk_call(move || async move {
        //             let mut properties = Properties::default();
        //             properties["len"] = property_value(len);
        //             Ok(properties)
        //         }))
        //     })
        //     // Configure execution to be async
        //     .enable_async();

        // Test executing thunk and reading properties
        // let _ = count_block_len_thunk
        //     .map_into(|call| {
        //         // Test executing the call
        //         let _call = call.clone();
        //         async move { _call.call().await }
        //     })
        //     .await
        //     .disable_async()
        //     .read(|properties| {
        //         // Test reading the result
        //         println!("{:?}", properties["len"]);
        //         Ok(())
        //     });

        let mut doc_builder = DocumentBuilder::new();
        compiler
            .update_last_build(&mut doc_builder)
            .enable_async()
            .read(|_| async {
                println!("Entering async");
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                Ok(())
            })
            .await
            .disable_async()
            .read(|_| {
                println!("Exiting async");
                Ok(())
            });

        let mut build_interner = Interner::default();
        compiler
            .update_last_build(&mut build_interner)
            .read(|interner| {
                println!("{:#?}", interner);
                Ok(())
            });

        let mut build_properties = Properties::new(Identifier::default());
        compiler
            .update_last_build(&mut build_properties)
            .read(|prop| {
                println!("{:#?}", prop);
                Ok(())
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

            b.search_index("input.(var)")
                .for_each(|(ident, mut interpolated, entity)| {
                    trace!("Installing thunk for input extension -- {:#}", ident);
                    if let Some(var) = interpolated.remove("var") {
                        compiler
                            .as_ref()
                            .write_component()
                            .insert(*entity, thunk_call(TestInput(var)))
                            .ok();
                    }
                })
        });
        compiler.as_mut().maintain();

        // Test query api
        for (ident, _, props) in compiler
            .compiled()
            .query("input.(var)", LHSOperator())
            .unwrap()
        {
            println!("{}", ident);
            println!("{:?}", props);
        }

        // Test command functions

        // Export toml doc
        export_toml(&mut compiler, ".test/test1.toml").await?;

        // Import toml doc
        if let Some(_) = import_toml(&mut compiler, ".test/test1.toml").await.ok() {
            export_toml(&mut compiler, ".test/test3.toml").await?;
        }

        Ok(())
    }

    #[derive(Copy, Clone)]
    struct LHSOperator();

    impl<'a> System<'a> for LHSOperator {
        type SystemData = (
            Read<'a, LazyUpdate>,
            Entities<'a>,
            ReadStorage<'a, BuildLog>,
        );

        fn run(&mut self, (lazy_update, entities, logs): Self::SystemData) {
            for (e, log) in (&entities, &logs).join() {
                let log = log.clone();

                // Test being able to use build_ref w/ System<'a>
                lazy_update.exec_mut(move |world| {
                    let mut world_ref = WorldWrapper::from(world);
                    let world_ref = &mut world_ref;

                    for (ident, _, _) in log.search_index("input.(var)") {
                        if let Some(build_ref) = log.find_ref::<Properties>(ident, world_ref) {
                            build_ref.read(|p| {
                                println!(
                                    "BuildLog: {}, Found properties for: {} {:#}",
                                    e.id(),
                                    p.owner(),
                                    p.owner()
                                );
                                Ok(())
                            });
                        }
                    }
                });
            }
        }
    }

    /*
        If feature(adt_const_params) then this could be written as --

        Operator<Expression::LHS>();
    */

    impl Predicate for LHSOperator {
        fn filter(
            self,
            _: &Identifier,
            interpolated: &BTreeMap<String, String>,
            properties: &Properties,
        ) -> bool {
            interpolated["var"] == "lhs"
                && properties["type"]
                    .as_symbol()
                    .filter(|s| *s == "stdin")
                    .is_some()
                && properties["rust_log"].as_symbol().is_some()
        }
    }

    struct TestInput(String);

    #[async_trait]
    impl Call for TestInput {
        async fn call(&self) -> Result<Properties, Error> {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            let mut result = Properties::default();
            result[&self.0] = property_value(100);

            result["t"] = property_list(vec![1.0, 2.0, 3.0, 4.0]);

            Ok(result)
        }
    }

    #[allow(dead_code)]
    #[derive(Deserialize, Debug)]
    struct TestInput2 {
        testing_update: bool,
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

    /// Component for a tokio task,
    ///
    #[derive(Component)]
    #[storage(VecStorage)]
    pub struct Task<T: Send + Sync + 'static> {
        /// Handle to a tokio runtime,
        ///
        handle: Handle,
        /// Join handle to running task,
        ///
        join_handle: Option<JoinHandle<T>>,
    }

    impl Task<Result<Properties, Error>> {
        /// Starts a call on the current task and returns a new task,
        ///
        pub fn start_call(&self, call: impl Call + Clone + 'static) -> Self {
            let call = call.clone();

            Self {
                handle: self.handle.clone(),
                join_handle: Some(self.handle.spawn(async move { call.call().await })),
            }
        }

        /// Returns true if this task is empty,
        ///
        pub fn is_empty(&self) -> bool {
            self.join_handle.is_some()
        }

        /// Returns true if the task is ready,
        ///
        /// If None, is returned that means this is an empty task
        ///
        pub fn is_ready(&self) -> Option<bool> {
            self.join_handle.as_ref().map(|j| j.is_finished())
        }
    }

    /// Call scheduler system implementation,
    ///
    pub struct Scheduler;

    impl<'a> System<'a> for Scheduler {
        type SystemData = (
            Entities<'a>,
            Read<'a, LazyUpdate>,
            ReadStorage<'a, ThunkCall>,
            WriteStorage<'a, Task<Result<Properties, Error>>>,
        );

        fn run(&mut self, (entities, lu, calls, mut tasks): Self::SystemData) {
            // Lazily dispatch any pending tasks, depends on .maintain()
            for (call, task) in (&calls, tasks.drain()).join() {
                let new_task = task.start_call(call.clone());

                lu.create_entity(&entities).with(new_task).build();
            }

            // Queue all tasks that are ready
            let mut ready = vec![];
            for (entity, call, task) in (&entities, calls.maybe(), &tasks).join() {
                if call.is_none() && task.is_ready().unwrap_or_default() {
                    ready.push(entity);
                }
            }

            // Get the result for any tasks that are ready
            for (entity, ready) in ready.iter().zip(ready.iter().filter_map(|r| tasks.remove(*r))) {
                if let Some(task) = ready.join_handle {
                    let result = ready.handle.block_on(async { task.await? });
                    
                    match result {
                        Ok(properties) => {
                            lu.insert(*entity, properties);
                        },
                        Err(err) => {
                            error!("Task encountered an error, {err}");
                        },
                    }
                }
            }
        }
    }
}
