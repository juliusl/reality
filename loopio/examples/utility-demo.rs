use std::time::Duration;

use async_trait::async_trait;
use loopio::{engine::Engine, prelude::{StdExt, PoemExt}};
use reality::{Workspace, prelude::*};

/// ```
/// <edit(..poem.engine-proxy)>
/// 
/// ```
/// 
#[tokio::main]
async fn main() {
    let utility_runmd = r#"
    ```runmd
    + .operation test_std_io
    <utility/loopio.ext.std.io>
    <..println>             Hello World
    <..read_text_file>      loopio/examples/test.txt
    <test>                  Hello World 2

    + .operation test_hyper
    <echo>                                              # Echoes an incoming request, Also schedules a shutdown
    <utility/loopio>                                    # Enable utilities
    <..hyper.request> http://test-engine-proxy/test     # Send outbound request

    + .operation test_poem
    <utility/loopio>
    <..poem.engine-proxy> localhost:0
    : .alias http://test-engine-proxy
    : test          .route test_std_io
    : test_handler  .route test_hyper
    : test          .get /test
    : test_handler  .get /test-handler/:name

    + .sequence start_tests
    : .next test_std_io
    : .next test_poem
    : .loop false
    ```
    "#;

    let mut workspace = Workspace::new();
    workspace.add_buffer("test_utilities.md", utility_runmd);

    let mut engine = Engine::builder();
    engine.register::<Test>();
    engine.register::<Echo>();
    let engine = engine.build();
    let engine = engine.compile(workspace).await;
    
    let mut s = None;
    for (seq, _seq) in engine.iter_sequences() {
        s = Some(_seq.clone());
        println!("{seq} {:?}", _seq);
    }
    tokio::spawn(async move { engine.handle_packets().await });
    let r = s.unwrap().await;
    assert!(r.is_err());
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
        let initialized = context.initialized::<Test>().await;
        
        let content = context.find_file_text("loopio/examples/test.txt");
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
        if let Some(req) = context.take_request() {
            println!("{:#?}", req.path);
            println!("{:#?}", req.uri);
            println!("{:#?}", req.headers);
        }

        let handle = context.engine_handle().unwrap();
        handle.shutdown(Duration::from_secs(4)).await?;
        
        Ok(())
    }
}
