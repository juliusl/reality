use std::time::Duration;

use async_trait::async_trait;
use bytes::BufMut;
use bytes::BytesMut;
use loopio::engine::Engine;
use loopio::foreground::ForegroundEngine;
use loopio::prelude::PoemExt;
use loopio::prelude::StdExt;

use loopio::prelude::flexbuffers_ext::FlexbufferExt;
use reality::prelude::*;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// Demo and test bed for utility plugins and extensions,
///
fn main() {
    std::env::set_var("RUST_LOG", "reality=info,loopio=debug");
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let utility_runmd = include_str!("utility-demo.md");

    let mut workspace = Workspace::new();
    workspace.add_buffer("test_utilities.md", utility_runmd);

    let mut engine = Engine::builder().enable_isolation();
    engine.enable::<Test>();
    engine.enable::<Echo>();
    engine.set_workspace(workspace);

    // let engine = engine.compile().await.unwrap();

    // let (eh, _) = engine.default_startup().await.unwrap();

    // let _ = eh.run("a").await.unwrap();

    let fg = ForegroundEngine::new(engine);

    if let Some(bg) = fg.engine_handle().background() {
        if let Some(mut opa) = bg.call("start_tests").ok() {
            opa.spawn();

            opa.into_foreground().unwrap();
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
        context.enable_flexbuffer().await;
        {
            let mut total_buf = BytesMut::new();
            context
                .write_flexbuffer(|b| {
                    b.start_map().push("name", "jello");
                    total_buf.put(b.view());
                })
                .await?;
        }

        let mut __name = Vec::new();
        context
            .read_flexbuffer(|r| {
                if let Some(name) = r.as_map().index("name").ok() {
                    assert_eq!("jello", name.as_str());
                    println!("reading from flexbuffer node -- {name}");
                    __name.push(name.as_str().to_string());
                }
            })
            .await?;

        // println!("Printing from outside -- {:?}", __name);
        let initialized = context.initialized::<Test>().await;

        let content = context.find_file_text("loopio/examples/test.txt").await;
        println!("{:?}", content);
        assert_eq!(initialized.expect.as_str(), content.unwrap_or_default());

        if let Some(result) = context.find_command_result("ls").await {
            println!("{}", String::from_utf8(result.output)?);
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
