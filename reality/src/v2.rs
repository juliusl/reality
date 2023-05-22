use specs::Component;
use tracing::trace;
use tracing::warn;

mod action;
pub use action::Action;

mod root;
pub use root::Root;

mod block;
pub use block::Block;

mod parser;
pub use parser::Parser;

mod compiler;
pub use compiler::linker::Linker;
pub use compiler::linker::LinkerEvents;
pub use compiler::BuildLog;
pub use compiler::Compiled;
pub use compiler::Compiler;
pub use compiler::CompilerEvents;
pub use compiler::DispatchRef;
pub use compiler::WorldWrapper;
pub mod states {
    pub use super::compiler::CompiledBuild as Build;
    pub use super::compiler::Object;
}

mod block_list;
pub use block_list::BlockList;

mod documentation;
pub use documentation::Documentation;

mod visitor;
pub use visitor::Visitor;

mod interner;
pub use interner::Interner;

mod properties;
pub use properties::property_list;
pub use properties::property_value;
pub use properties::Properties;
pub use properties::Property;

pub mod thunk;
pub use thunk::thunk_build;
pub use thunk::thunk_call;
pub use thunk::thunk_compile;
pub use thunk::thunk_listen;
pub use thunk::thunk_update;
pub use thunk::Accept;
pub use thunk::AsyncDispatch;
pub use thunk::Build;
pub use thunk::Call;
pub use thunk::Dispatch;
pub use thunk::DispatchResult;
pub use thunk::DispatchThunkBuild;
pub use thunk::Listen;
pub use thunk::Listener;
pub use thunk::Thunk;
pub use thunk::ThunkBuild;
pub use thunk::ThunkCall;
pub use thunk::ThunkCompile;
pub use thunk::ThunkListen;
pub use thunk::ThunkUpdate;
pub use thunk::Update;

use crate::Identifier;

use self::prelude::Load;
use self::prelude::Provider;
use self::prelude::Visit;

mod data;
pub mod toml {
    pub use crate::v2::data::toml::DocumentBuilder;
}

pub mod command;
pub mod prelude;

/// Helper trait for pattern matching w/ a build log,
///
pub trait GetMatches
where
    Self: Sized + Clone,
{
    /// Returns a vector of pattern matches from build log,
    ///
    fn get_matches(build_log: &BuildLog) -> Vec<(Identifier, Self, specs::Entity)> {
        build_log
            .index()
            .iter()
            .flat_map(|(i, e)| {
                Self::get_match(i)
                    .iter()
                    .map(|m| (i.clone(), m.clone(), *e))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }

    fn get_match(ident: &Identifier) -> Vec<Self>;
}

impl GetMatches for () {
    fn get_matches(_: &BuildLog) -> Vec<(Identifier, Self, specs::Entity)>
    where
        Self: Sized,
    {
        Vec::new()
    }

    fn get_match(_: &Identifier) -> Vec<Self>
    where
        Self: Sized,
    {
        vec![]
    }
}

/// Enumeration of instance states for an entity,
///
#[derive(Component, Debug)]
#[storage(specs::storage::DenseVecStorage)]
pub enum Instance {
    /// The instance has been compiled and has all thunks applied,
    ///
    Ready,
    // TODO -- Should this be used to track state?
}

/// Trait to implement to extend a runmd compiler,
///
pub trait Runmd: Dispatch + Visitor + Component + Clone + Send + Sync
where
    for<'a> &'a Self: Visit,
    <Self as Component>::Storage: Default,
{
    /// Associated type for the loadable instance type,
    /// 
    type Instance<'a>: Load + 'a;
    
    /// Associated type for the instance system data type,
    /// 
    type InstanceSystemData<'a>: Provider<'a, <Self::Instance<'a> as Load>::Layout>;

    /// Associated type for the runmd linker type,
    /// 
    type Linker: for<'a> Visit<CompilerEvents<'a, Self>> + GetMatches + std::fmt::Debug;

    /// Name of the concrete type,
    /// 
    /// **Note** The lowercase version of the name will be used as the custom attribute symbol,
    /// 
    fn type_name() -> &'static str;

    /// Finishes building runmd type w/ compiler,
    ///
    fn runmd(&self, compiler: &mut Compiler) -> Result<(), crate::Error> {
        use specs::Entities;
        use specs::Join;
        use specs::LazyUpdate;
        use specs::Read;
        use specs::ReadStorage;
        use specs::WorldExt;

        compiler.as_mut().exec(
            |(lz, entities, identifiers, linker_events): (
                Read<LazyUpdate>,
                Entities,
                ReadStorage<Identifier>,
                ReadStorage<LinkerEvents<Self>>,
            )| {
                for (e, ident, event) in (&entities, &identifiers, &linker_events).join() {
                    match event {
                        LinkerEvents::Ready(c) => {
                            let c = c.clone();
                            lz.exec_mut(move |w| {
                                let mut wrapper = WorldWrapper::from(w);
                                let mut disp = wrapper.get_ref(e);
                                disp.store(c.clone()).expect("should be able to insert component");

                                c.dispatch(disp)
                                    .expect("should be able to dispatch");
                            });
                            trace!(
                                ident = format!("{:#}", ident), 
                                entity_id=e.id(), 
                                entity_gen=e.gen().id(), 
                                "Completed building"
                            );
                        }
                        _ => {
                            warn!(
                                ident = format!("{:#}", ident), 
                                entity_id=e.id(), 
                                entity_gen=e.gen().id(),
                                "Unhandled linker events"
                            );
                            continue;
                        }
                    }
                }
            },
        );
        compiler.as_mut().maintain();
        Ok(())
    }
}

#[allow(unused_variables)]
#[allow(unused_imports)]
#[allow(dead_code)]
mod tests {
    use crate::{
        v2::{
            data::{
                query::{all, Predicate, Query},
                toml::TomlProperties,
            },
            toml::DocumentBuilder,
            Compiler, Parser, Properties,
        },
        Identifier,
    };

    use super::Visitor;

    const EXAMPLE_PLUGIN: &'static str = r##"
```runmd
+ .plugin                               # Extensions that can be used when defining a plugin
<> .path                                # Indicates that the variable should be a path
: canonical .bool                       # If enabled, will check if the value is a canonical path
: cache     .bool                       # If enabled, indicates that the file at path should be read
<> .map                                 # Indicates that the variable will have key-value pairs within the root
<> .list                                # Indicates that the variable can be a list of values

+ .plugin process                       # Plugin that executes a child process
: cache_output 	.bool 	                # Caches output from process to a property
: silent		.bool 	                # Silences stdout/stderror from process to parent
: inherit		.bool	                # Inherits any arg/env values from parent's properties
: redirect		.symbol                 # Redirects output from process to path
: cd			.symbol	                # Sets the current directory of the process to path
: env			.symbol	                # Map of environment variables to set before starting the process
: arg			.symbol	                # List of arguments to pass to the process
: flag		    .symbol	                # List of flags to pass to the process

<path>  .redirect : canonical .true     # Should be a canonical path
<path>  .cd                             # Should be a path
<map>   .env                            # Should be a map
<list>  .arg                            # Should be a list
<list>  .flag                           # Should be a list
```

```runmd app
+ .runtime
<plugin> 	.process    cargo test
: RUST_LOG 	.env        reality=trace
:           .arg	    --package
:           .arg        reality
:           .redirect   .test/test.output
```
"##;

    #[tokio::test]
    async fn test_example_plugin() {
        let mut compiler = Compiler::new().with_docs();

        let _ = Parser::new()
            .parse(EXAMPLE_PLUGIN, &mut compiler)
            .expect("should parse");

        let _ = compiler.compile().expect("should compile");

        let log = compiler.last_build_log().unwrap();

        log.find_ref::<Properties>(".plugin.process.path.redirect", &mut compiler)
            .unwrap()
            .read(|props| {
                assert!(props["canonical"].is_enabled());
                Ok(())
            });

        log.find_ref::<Properties>(".plugin.process.path.cd", &mut compiler)
            .unwrap()
            .read(|props| {
                assert!(!props["canonical"].is_enabled());
                Ok(())
            });

        let mut doc = DocumentBuilder::new();
        compiler
            .update_last_build(&mut doc)
            .map_into::<TomlProperties>(|d| Ok(d.into()))
            .read(|props| {
                println!("{}", props.doc);

                for (ident, map, _) in props
                    .all("runtime.plugin.(plugin).(input)")
                    .expect("should be able to query")
                {
                    println!("{:#}", ident);
                    println!("Plugin: {:?}", map.get("plugin"));
                    println!("Input:  {:?}", map.get("input"));
                }

                Ok(())
            });
    }
}
