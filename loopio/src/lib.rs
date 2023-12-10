pub mod action;
pub mod address;
pub mod background_work;
pub mod deck;
pub mod engine;
mod ext;
pub mod foreground;
pub mod host;
pub mod operation;
pub mod prelude;
pub mod sequence;

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
    use tokio::pin;
    use tokio::sync::Notify;
    use tracing::trace;
    use uuid::Bytes;

    use crate::engine::Engine;
    use crate::engine::EngineBuilder;
    use crate::operation::Operation;
    use crate::prelude::Action;
    use crate::prelude::Address;
    use crate::prelude::VirtualBusExt;

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
                tc.attribute.key()
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
                tc.attribute.key()
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
    #[tracing_test::traced_test]
    async fn test_plugin_model() {
        let mut builder = Engine::builder();
        builder.enable::<TestPlugin>();
        builder.enable::<TestPlugin2>();

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
        <b/demo.test_plugin2> cargo
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

        let mut workspace = Workspace::new();
        workspace.add_buffer(".test/test_plugin.md", runmd);

        let engine = engine.compile(workspace).await;
        let eh = engine.engine_handle();

        engine.spawn(|_, p| Some(p));

        if let Ok(_resource) = eh.hosted_resource("engine://start#test").await {
            // Create a new virtual bus
            let mut bus = _resource
                .context()
                .virtual_bus(Address::from_str("test/operation#test_tag").unwrap())
                .await;

            // Create a clone for the test task
            let mut txbus = bus.clone();

            let _ = tokio::spawn(async move {
                let tx = txbus.transmit::<TestPlugin2>().await;

                tx.write_to_virtual(|virt| {
                    eprintln!("writing to virtual");
                    virt.virtual_mut().name.commit()
                });
            });

            // Create a new port listening for changes to the name field
            let mut bus_port = bus
                .wait_for::<TestPlugin2>()
                .await
                .select(|s| &s.virtual_ref().name)
                .filter(|f| f.is_committed())
                .pinned();

            if let Some(_) = bus_port.deref_mut().next().await {
                eprintln!("got update");
            }
        }
        ()
    }
}
