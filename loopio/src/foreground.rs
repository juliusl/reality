use anyhow::Error;
use reality::ResourceKey;
use reality::StorageTarget;
use tokio::task::JoinError;
use tokio::task::JoinHandle;

use crate::background_work::BackgroundWork;
use crate::background_work::BackgroundWorkEngineHandle;
use crate::background_work::CallStatus;
use crate::engine::EngineBuilder;
use crate::prelude::Engine;
use crate::prelude::EngineHandle;
use crate::prelude::Published;
// use crate::prelude::Published;

/// Type-alias for the background task listening for new engine packets,
///
pub type EngineListenerBackgroundTask = JoinHandle<Result<Result<Engine, Error>, JoinError>>;

/// Engine meant for foreground thread usage,
///
/// Cannot be created inside of the context of another tokio runtime.
///
pub struct ForegroundEngine {
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
# -- # Test the background work
+ .operation test_background_work
<loopio.std.io.println> Hello world a

# -- # Default engine operation plugins
+ .operation default
<handle/loopio.background-work>
<list/loopio.published>

# -- # Default host engine tasks
+ .host engine

# -- # Creates a new background work engine handle
: .action   default/handle/loopio.background-work

# -- # Retrieves current published resources
: .action   default/list/loopio.published
```
"#,
                );
                builder.enable::<BackgroundWork>();
                builder.enable::<Published>();
                builder.compile().await
            })
            .unwrap();

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
        if let Ok(mut bg) = bg.call("engine://test_background_work") {
            loop {
                match bg.status() {
                    CallStatus::Enabled => {
                        bg.spawn();
                    }
                    CallStatus::Disabled => {
                        eprintln!("disabled");
                        break;
                    }
                    CallStatus::Running => std::thread::yield_now(),
                    CallStatus::Pending => {
                        bg.clone().into_foreground().ok();
                        break;
                    }
                }
            }
        }

        ForegroundEngine {
            eh,
            runtime,
            __engine_listener,
        }
    }
}

#[test]
#[tracing_test::traced_test]
fn test_foreground_engine() {
    let mt_engine = ForegroundEngine::new(crate::prelude::Engine::builder());

    if let Some(bg) = mt_engine.engine_handle().background() {
        let mut bg = bg.call("test_background_work").unwrap();
        bg.spawn();
        let _ = bg.into_foreground().unwrap();
    }
}
