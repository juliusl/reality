mod action;
pub use action::Action;

mod root;
pub use root::Root;

mod block;
pub use block::Block;

mod parser;
pub use parser::Parser;

mod compiler;
pub use compiler::BuildRef;
pub use compiler::Compiler;
pub use compiler::Object;

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

mod thunk;
pub use thunk::thunk_build;
pub use thunk::thunk_call;
pub use thunk::thunk_compile;
pub use thunk::thunk_listen;
pub use thunk::thunk_update;
pub use thunk::Accept;
pub use thunk::Build;
pub use thunk::Call;
pub use thunk::Compile;
pub use thunk::Listen;
pub use thunk::Listener;
pub use thunk::Thunk;
pub use thunk::ThunkBuild;
pub use thunk::ThunkCall;
pub use thunk::ThunkCompile;
pub use thunk::ThunkListen;
pub use thunk::Update;

mod links;
pub use links::Link;
pub use links::Links;

mod data;
pub mod toml {
    pub use crate::v2::data::toml::DocumentBuilder;
}

pub mod command;

#[allow(unused_imports)]
#[allow(dead_code)]
mod tests {
    use crate::v2::{
        data::{
            query::{all, Predicate, Query},
            toml::TomlProperties,
        },
        toml::DocumentBuilder,
        Compiler, Parser,
    };

    const EXAMPLE_PLUGIN: &'static str = r##"
```runmd
+ .plugin Process                       # Plugin that executes a child process
: cache_output 	.bool 	                # Caches output from process to a property
: silent		.bool 	                # Silences stdout/stderror from process to parent
: inherit		.bool	                # Inherits any arg/env values from parent's properties
: redirect		.symbol                 # Redirects output from process to path
: cd			.symbol	                # Sets the current directory of the process to path
: env			.symbol	                # Map of environment variables to set before starting the process
: arg			.symbol	                # List of arguments to pass to the process
: flag		    .symbol	                # List of flags to pass to the process

# Extensions
<path>  .redirect : canonical .true     # Must be a canonical path
<path>  .cd                             # Optionally a canonical path
<map>   .env                            # Name is the environment variable name and the value is the environment variable value
<list>  .arg                            # List of arguments to pass
<list>  .flag                           # List of flags to pass

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

        let mut doc = DocumentBuilder::new();
        compiler
            .update_last_build(&mut doc)
            .map_into::<TomlProperties>(|d| Ok(d.into()))
            .read(|props| {
                println!("{}", props.doc);

                for (ident, map, _) in props
                    .all("plugin.process.(cmd)")
                    .expect("should be able to query")
                {
                    println!("{:#}", ident);
                    println!("{:?}", map.get("cmd"));
                }

                Ok(())
            });
    }
}
