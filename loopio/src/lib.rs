pub mod engine;
mod ext;
pub mod host;
pub mod operation;
pub mod prelude;
pub mod sequence;
pub mod address;
pub mod action;
pub mod spawned;
pub mod deck;
pub mod background_work;
pub mod foreground;

#[allow(unused_imports)]
mod tests {
    use std::clone;
    use std::collections::BTreeMap;
    use std::marker::PhantomData;
    use std::ops::Deref;
    use std::sync::Arc;
    use std::time::Duration;

    use async_stream::try_stream;
    use futures_util::{pin_mut, StreamExt, TryStreamExt};
    use reality::derive::*;
    use reality::prelude::*;
    use tokio::io::AsyncReadExt;
    use tokio::join;
    use tracing::trace;
    use uuid::Bytes;

    use crate::engine::Engine;
    use crate::engine::EngineBuilder;
    use crate::operation::Operation;

    #[derive(Reality, Default, Debug, Clone)]
    #[reality(plugin, group = "demo", rename = "test_plugin2")]
    struct TestPlugin2 {
        #[reality(derive_fromstr)]
        _process: String,
        name: String,
        #[reality(map_of=String)]
        env: BTreeMap<String, String>,
        #[reality(vec_of=String)]
        args: Vec<String>,
    }

    #[derive(Reality, Default, Debug, Clone)]
    #[reality(plugin, group = "demo", rename = "test_plugin")]
    struct TestPlugin {
        #[reality(derive_fromstr)]
        _process: String,
        name: String,
        #[reality(map_of=String)]
        env: BTreeMap<String, String>,
        #[reality(vec_of=String)]
        args: Vec<String>,
    }

    #[async_trait::async_trait]
    impl CallAsync for TestPlugin {
        async fn call(tc: &mut ThunkContext) -> anyhow::Result<()> {
            let _initialized = tc.initialized::<TestPlugin>().await;
            println!(
                "Initialized as -- {:?} {:?}",
                _initialized,
                tc.attribute.map(|a| a.key())
            );

            if tc.variant_id.is_some() {
                let frame = _initialized.to_frame(tc.attribute);
                println!("{:?}", frame);
            }

            println!("Tag: {:?}", tc.tag().await);

            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl CallAsync for TestPlugin2 {
        async fn call(tc: &mut ThunkContext) -> anyhow::Result<()> {
            let _initialized = tc.initialized::<TestPlugin>().await;
            println!(
                "Initialized as -- {:?} {:?}",
                _initialized,
                tc.attribute.map(|a| a.key())
            );

            if tc.variant_id.is_some() {
                let frame = _initialized.to_frame(tc.attribute);
                println!("{:?}", frame);
            }

            println!("Tag: {:?}", tc.tag().await);

            Ok(())
        }
    }

    #[tokio::test]
    // #[tracing_test::traced_test]
    async fn test_plugin_model() {
        // TODO: Test Isoloation -- 7bda126d-466c-4408-b5b7-9683eea90b65
        let mut builder = Engine::builder();
        builder.enable::<TestPlugin>();

        let engine = builder.build();
        let runmd = r#"
        ```runmd
        + .operation test/operation
        <test/demo.test_plugin> cargo
        : .name hello-world-3
        : RUST_LOG .env lifec=trace
        : HOME .env /home/test2
        : .args --name
        : .args test3

        + test_tag .operation test/operation
        <a/demo.test_plugin> cargo
        : .name hello-world-2-tagged
        : RUST_LOG .env lifec=debug
        : HOME .env /home/test
        : .args --name
        : .args test
        <b/demo.test_plugin> cargo
        : .name hello-world-3-tagged
        : RUST_LOG .env lifec=trace
        : HOME .env /home/test2
        : .args --name
        : .args test3

        + test .sequence start
        : .next 'test/operation#test_tag'
        : .next test/operation
        : .loop false
        ```
        "#;

        tokio::fs::create_dir_all(".test").await.unwrap();

        tokio::fs::write(".test/test_plugin.md", runmd)
            .await
            .unwrap();

        let mut workspace = Workspace::new();
        workspace.add_local(".test/test_plugin.md");

        let engine = engine.compile(workspace).await;
        let eh = engine.engine_handle();

        if let Ok(resource) = eh.hosted_resource("engine://state#test").await {

        }

        // for (address, _) in engine.iter_operations() {
        //     println!("{address}");
        // }

        // let mut sequences = engine.iter_sequences().collect::<Vec<_>>().clone();
        // let mut _seq = None;
        // if let Some((address, seq)) = sequences.pop() {
        //     println!("{address} -- {:#?}", seq);

        //     _seq = Some(seq.clone());
        //     tokio::spawn(async move { engine.handle_packets(|_, packet| Some(packet)).await });
        // }

        // _seq.clone().unwrap().await.unwrap();
        // _seq.unwrap().await.unwrap();

        ()
    }
}
