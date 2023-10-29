use async_trait::async_trait;
use loopio::{engine::Engine, prelude::StdExt};
use reality::{Workspace, prelude::*};

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
    <utility/loopio.hyper.request> http://localhost:5678/test

    + .operation test_poem
    <utility/loopio.poem.engine-proxy> localhost:5678
    : test          .route test_std_io
    : test_handler  .route test_hyper
    : test          .get /test
    : test_handler  .get /test-handler
    ```
    "#;

    let mut workspace = Workspace::new();
    workspace.add_buffer("test_std_io.md", utility_runmd);

    let mut engine = Engine::builder();
    engine.register::<Test>();
    let engine = engine.build();
    let engine = engine.compile(workspace).await;
    
    engine.run("test_std_io").await.unwrap();

    engine.run("test_poem").await.unwrap();
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