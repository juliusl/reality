use std::time::Duration;

use async_trait::async_trait;
use loopio::engine::Engine;
use loopio::foreground::ForegroundEngine;
use loopio::prelude::PoemExt;
use loopio::prelude::StdExt;

use loopio::prelude::flexbuffers_ext::FlexbufferCacheExt;
use reality::prelude::*;

/// Demo and test bed for utility plugins and extensions,
///
fn main() {
    loopio::setup_logging(loopio::LoggingLevel::Default);

    let utility_runmd = include_str!("utility-demo.md");

    let mut workspace = Workspace::new();
    workspace.add_buffer("test_utilities.md", utility_runmd);

    let mut engine = Engine::builder();
    engine.enable::<Test>();
    engine.enable::<Echo>();
    engine.set_workspace(workspace);

    let fg = ForegroundEngine::new(engine);

    if let Some(bg) = fg.engine_handle().background() {
        if let Some(mut tests) = bg.call("start_tests").ok() {
            assert!(tests.spawn().is_running());

            assert!(
                tests.into_foreground().is_err(),
                "should return an error to shutdown"
            );
        }
    }
    ()
}

#[derive(Reality, Default, Clone)]
#[reality(plugin, rename = "test", group = "user")]
struct Test {
    #[reality(derive_fromstr)]
    expect: String,
}

#[async_trait]
impl CallAsync for Test {
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        // Test flexbuffer api
        context
            .flexbuffer_scope()
            .build(|mut b| b.start_map().push("name", "jello"));

        // Test that the update was persisted on drop
        {
            let reader = context.flexbuffer_view().expect("should be enabled");
            let value = reader.as_map().index("name").ok().map(|v| v.as_str());
            eprintln!("{:?}", value);
            assert_eq!(Some("jello"), value);
        }

        let initialized = context.initialized::<Test>().await;

        // Test that the read-text-file result is found from transient storage
        let content = context.find_file_text("loopio/examples/test.txt").await;
        println!("{:?}", content);
        assert_eq!(initialized.expect.as_str(), content.unwrap_or_default());

        // Test the command result can be found from transient storage
        if let Some(result) = context.find_command_result("ls").await {
            println!("{}", String::from_utf8(result.output)?);
        }

        // Test the request is passed from the reverse/engine proxy
        if let Some(request) = context.take_request().await {
            eprintln!("{:#?}", request.path);
        }

        Ok(())
    }
}

#[derive(Reality, Default, Clone)]
#[reality(plugin, group = "user")]
struct Echo {
    #[reality(derive_fromstr)]
    unused: String,
}

#[async_trait]
impl CallAsync for Echo {
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        use loopio::prelude::Ext;
        if let Some(req) = context.take_request().await {
            println!("{:#?}", req.path);
            println!("{:#?}", req.uri);
            println!("{:#?}", req.headers);
        }

        let handle = context.engine_handle().await.unwrap();
        handle.shutdown(Duration::from_secs(4)).await?;

        Ok(())
    }
}

#[test]
fn test_symbols() {
    println!("{}", <Test as AttributeType<Shared>>::symbol());
    println!(
        "{}",
        <loopio::prelude::Process as AttributeType<Shared>>::symbol()
    )
}
