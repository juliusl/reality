use std::time::Duration;

use async_trait::async_trait;
use loopio::{engine::Engine, prelude::{StdExt, PoemExt}};
use reality::{Workspace, prelude::*};

/// Demo and test bed for utility plugins and extensions,
/// 
#[tokio::main]
async fn main() {
    let utility_runmd = r#"
    ```runmd
    + .operation test_std_io
    <utility/loopio.ext.std.io>
    <..println>             Hello World                     # Prints a new line
    <..read_text_file>      loopio/examples/test.txt        # Read a text file into transient storage
    <test>                  Hello World 2                   # Verifies the file

    + .operation test_hyper
    <echo>                                                   # Echoes an incoming request, Also schedules a shutdown
    <utility/loopio>                                         # Enable utilities
    <..hyper.request> testhost://test-engine-proxy/test      # Send outbound request

    + .operation test_poem
    <utility/loopio>
    <..poem.engine-proxy> localhost:0
    : .alias testhost://test-engine-proxy
    : test          .route test_std_io
    : test_2        .route run_println
    : test_handler  .route test_hyper
    : test          .get /test
    : test_2        .get /test2
    : test_handler  .get /test-handler/:name

    + .sequence start_tests
    : .next test_std_io
    : .next test_poem
    : .loop false

    + .sequence run_println
    : .next test_std_io
    : .loop false

    + .host testhost
    : .start start_tests
    ```
    "#;

    let mut workspace = Workspace::new();
    workspace.add_buffer("test_utilities.md", utility_runmd);

    let mut engine = Engine::builder();
    engine.register::<Test>();
    engine.register::<Echo>();
    let engine = engine.build();
    let engine = engine.compile(workspace).await;
    
    let host = engine.get_host("testhost").expect("should have host");
    tokio::spawn(async move { 
        engine.handle_packets().await 
    });
   
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

        let comments = context.get_comments();
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
        if let Some(req) = context.take_request() {
            println!("{:#?}", req.path);
            println!("{:#?}", req.uri);
            println!("{:#?}", req.headers);
        }

        let comments = context.get_comments();
        println!("{:#?}", comments);

        let handle = context.engine_handle().unwrap();
         handle.shutdown(Duration::from_secs(4)).await?;
        
        Ok(())
    }
}
