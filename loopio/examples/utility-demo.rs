use std::time::Duration;

use async_trait::async_trait;
use loopio::prelude::StdExt;
use loopio::prelude::PoemExt;
use loopio::engine::Engine;

use reality::prelude::*;

/// Demo and test bed for utility plugins and extensions,
///
#[tokio::main]
async fn main() {
    let utility_runmd = include_str!("utility-demo.md");

    let mut workspace = Workspace::new();
    workspace.add_buffer("test_utilities.md", utility_runmd);

    let mut engine = Engine::builder();
    engine.register::<Test>();
    engine.register::<Echo>();
    let engine = engine.build();
    let engine = engine.compile(workspace).await;

    let host = engine.get_host("testhost").expect("should have host");

    tokio::spawn(async move { engine.handle_packets().await });

    let result = host.start().await;
    assert!(result.is_err());
    ()
}

#[derive(Reality, Default, Clone)]
#[reality(plugin, rename = "test")]
struct Test {
    #[reality(derive_fromstr)]
    expect: String,
}

#[async_trait]
impl CallAsync for Test {
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        use loopio::prelude::Ext;
        let initialized = context.initialized::<Test>().await;

        let comments = context.get_comments().await;
        println!("{:#?}", comments);

        let content = context.find_file_text("loopio/examples/test.txt").await;
        println!("{:?}", content);
        assert_eq!(initialized.expect.as_str(), content.unwrap_or_default());
        Ok(())
    }
}

#[derive(Reality, Default, Clone)]
#[reality(plugin, rename = "echo")]
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

        let comments = context.get_comments().await;
        println!("{:#?}", comments);

        let handle = context.engine_handle().await.unwrap();
        handle.shutdown(Duration::from_secs(4)).await?;

        Ok(())
    }
}
