mod bootstrap;
pub use bootstrap::Bootstrap;

mod action;
pub use action::Action;
pub use action::ActionBuffer;

mod root;
pub use root::Root;

mod block;
pub use block::Block;

mod parser;
pub use parser::Parser;

mod compiler;
pub use compiler::linker::Linker;
pub use compiler::BuildLog;
pub use compiler::Compiled;
pub use compiler::Compiler;
pub use compiler::DispatchRef;
pub use compiler::WorldWrapper;
pub mod states {
    pub use super::compiler::CompiledBuild as Build;
    pub use super::compiler::Object;
}

pub mod framework;
pub use framework::Framework;

mod block_list;
pub use block_list::BlockList;

mod documentation;
pub use documentation::Documentation;

mod visitor;
use specs::Component;
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
pub use thunk::DispatchSignature;
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
    fn get_matches(build_log: &BuildLog) -> Vec<(Self, specs::Entity)> {
        build_log
            .index()
            .iter()
            .flat_map(|(i, e)| Self::get_match(i).iter().map(|m| (m.clone(), *e)).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    }

    fn get_match(ident: &Identifier) -> Vec<Self>;
}

impl GetMatches for () {
    fn get_matches(_: &BuildLog) -> Vec<(Self, specs::Entity)>
    where
        Self: Sized,
    {
        Vec::new()
    }

    fn get_match(ident: &Identifier) -> Vec<Self>
    where
        Self: Sized,
    {
        todo!()
    }
}

/// Trait to implement to extend a runmd compiler,
///
pub trait Runmd: Visitor + Component + Clone + Send + Sync {
    /// Associated type for pattern matching w/ a build log,
    ///
    type Extensions: GetMatches + std::fmt::Debug;

    /// Configures the compiler for a runmd-based project,
    ///
    fn runmd(&self, compiler: &mut Compiler) -> Result<(), crate::Error>;
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
