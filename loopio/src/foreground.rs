use std::time::Duration;

use anyhow::Error;
use reality::prelude::*;
use tokio::task::JoinError;
use tokio::task::JoinHandle;
use tracing::trace;

use crate::background_work::BackgroundWork;
use crate::background_work::BackgroundWorkEngineHandle;
use crate::background_work::DefaultController;
use crate::engine::EngineBuilder;
use crate::prelude::Engine;
use crate::prelude::EngineHandle;
use crate::prelude::Published;
use crate::work::PrivateProgress;
use crate::work::WorkState;

/// Type-alias for the background task listening for new engine packets,
///
pub type EngineListenerBackgroundTask = JoinHandle<Result<Result<Engine, Error>, JoinError>>;

/// Engine meant for foreground thread usage,
///
/// Cannot be created inside of the context of another tokio runtime.
///
pub struct ForegroundEngine {
    /// Compiled package,
    /// 
    pub package: Package,
    /// Engine handle to the main engine,
    ///
    eh: EngineHandle,
    /// Tokio runtime this engine was initialized on,
    ///
    runtime: tokio::runtime::Runtime,
    /// Background task managing the engine listener,
    ///
    __engine_listener: EngineListenerBackgroundTask,
}

impl ForegroundEngine {
    /// Returns a new engine handle,
    ///
    pub fn engine_handle(&self) -> EngineHandle {
        self.eh.clone()
    }

    /// Returns a new tokio runtime handle,
    ///
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }

    /// Returns a new mutable reference to a tokio runtime,
    ///
    pub fn runtime_mut(&mut self) -> &mut tokio::runtime::Runtime {
        &mut self.runtime
    }

    /// Returns a reference to a tokio runtime,
    ///
    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.runtime
    }

    /// Creates a new foreground engine from a workspace,
    ///
    pub fn new(mut builder: EngineBuilder) -> ForegroundEngine {
        let runtime = builder.runtime_builder.build().unwrap();

        let engine = runtime
            .block_on(async {
                // Create/Test engine plugins
                builder.workspace.add_buffer(
                    "background-work.md",
                    r#"
```runmd
# -- Background work test operation
# -- Tests that the background-work system is functioning properly.
+ .operation test_background_work
|# internal = true

<test/loopio.foreground-engine-test> Hello world from background engine.

# -- Default engine operation plugins
# -- Initializes components to enable the background work system.
+ .operation default
|# internal = true

<handle/loopio.background-work>
<list/loopio.published>

# -- Default engine host
+ .host engine
|# internal = true

```
"#,
                );
                builder.enable::<BackgroundWork>();
                builder.enable::<Published>();
                builder.enable::<ForegroundEngineTest>();

                trace!("Compiling workspace {:#?}", builder.workspace);
                builder.compile().await
            })
            .unwrap();

        let package = engine.package.clone().expect("should have compiled a package");
        let mut eh = engine.engine_handle();

        let __engine_listener = runtime.spawn(async move {
            let (_, pk) = engine.default_startup().await.unwrap();
            pk.await
        });

        eh = runtime.block_on(async move {
            let tc = eh.run("engine://default").await.unwrap();
            let transient = tc.transient().await;
            let handle =
                transient.current_resource::<BackgroundWorkEngineHandle>(ResourceKey::root());
            assert!(handle.is_some());
            eh.background_work = Some(handle.unwrap());
            eh
        });

        // Run diagnostics before returning the foreground engine
        let bg = eh
            .background()
            .expect("should be able to create a background handle");

        // This tests that the bg engine is working properly
        if let Ok(mut bg) = bg.call("test_background_work/test/loopio.foreground-engine-test") {
            let mut controller = DefaultController;
            let progress = bg
                .as_ref()
                .cached_ref::<PrivateProgress>()
                .as_deref()
                .cloned();
            let _ = runtime.handle().spawn(async move {
                if let Some(progress) = progress {
                    trace!("Validating foreground engine work-state system");
                    progress
                        .listen_value(|v| {
                            assert_eq!(0.5, v.0);
                        })
                        .await?;

                    progress
                        .listen_value(|v| {
                            assert_eq!(0.7, v.0);
                        })
                        .await?;

                    progress
                        .listen_value(|v| {
                            assert_eq!(1.0, v.0);
                        })
                        .await?;
                }
                Ok::<_, anyhow::Error>(())
            });

            let _ = bg
                .wait_for_completion(&mut controller)
                .expect("should be able to complete");

            // runtime.handle().block_on(jh).expect("should be able to join").expect("should be able to complete");
        }

        ForegroundEngine {
            package,
            eh,
            runtime,
            __engine_listener,
        }
    }
}

/// Tests foreground engine,
///
#[derive(Reality, Clone, Debug, Default)]
#[plugin_def(call = run_foreground_engine_test)]
#[parse_def(rename = "foreground-engine-test")]
struct ForegroundEngineTest {
    #[reality(derive_fromstr)]
    name: String,
    test_progress: f32,
}

async fn run_foreground_engine_test(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<ForegroundEngineTest>().await;
    trace!("running foreground engine test -- {:?}", init);

    tc.set_progress(0.5);
    tc.set_message(init.name);
    let _ = <crate::work::PrivateWorkState as Plugin>::call(tc.clone()).await?;
    tc.node
        .runtime
        .clone()
        .unwrap()
        .spawn(async { tokio::time::sleep(Duration::from_millis(100)).await })
        .await?;

    tc.set_progress(0.7);
    let _ = <crate::work::PrivateWorkState as Plugin>::call(tc.clone()).await?;
    tc.node
        .runtime
        .clone()
        .unwrap()
        .spawn(async { tokio::time::sleep(Duration::from_millis(100)).await })
        .await?;

    tc.set_progress(1.0);
    let _ = <crate::work::PrivateWorkState as Plugin>::call(tc.clone()).await?;
    tc.node
        .runtime
        .clone()
        .unwrap()
        .spawn(async { tokio::time::sleep(Duration::from_millis(100)).await })
        .await?;

    Ok(())
}

#[test]
#[tracing_test::traced_test]
fn test_foreground_engine() {
    use crate::work::WorkState;
    use tower::Service;

    // Test self diagnosis works
    let engine = ForegroundEngine::new(crate::prelude::Engine::builder());

    // Verify background worker works
    if let Some(bg) = engine.engine_handle().background() {
        let tc = bg.tc.clone();

        let mut worker = bg
            .worker(ForegroundEngineTest {
                name: String::from("Hello world from background worker."),
                test_progress: 0.0,
            })
            .unwrap();

        futures::executor::block_on(async move {
            let result = worker.call(tc).await.unwrap();
            assert_eq!(result.get_progress(), Some(1.0));
            assert_eq!(
                result.get_message(),
                Some("Hello world from background worker.".to_string())
            );
        });
    }
}
