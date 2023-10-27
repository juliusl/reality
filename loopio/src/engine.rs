use std::collections::BTreeMap;
use std::sync::Arc;

use reality::prelude::*;
use tokio_util::sync::CancellationToken;

use anyhow::anyhow;
use tracing::info;

use crate::operation::Operation;
use crate::plugin::Plugin;
use crate::plugin::Thunk;
use crate::plugin::ThunkContext;

pub struct EngineBuilder {
    /// Plugins to register w/ the Engine
    ///
    plugins: Vec<reality::BlockPlugin<Shared>>,
    /// Runtime builder,
    ///
    runtime_builder: tokio::runtime::Builder,
}

impl EngineBuilder {
    /// Creates a new engine builder,
    ///
    pub fn new(runtime_builder: tokio::runtime::Builder) -> Self {
        Self {
            plugins: vec![],
            runtime_builder,
        }
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn register<P: Plugin + Send + Sync + 'static>(&mut self) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn register_extension<
        C: ExtensionController<P> + Send + Sync + 'static,
        P: Plugin + Send + Sync + 'static,
    >(
        &mut self,
    ) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<ExtensionPlugin<C, P>>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    #[inline]
    pub fn register_with(&mut self, plugin: fn(&mut AttributeParser<Shared>)) {
        self.plugins.push(Arc::new(plugin));
    }

    /// Consumes the builder and returns a new engine,
    ///
    pub fn build(mut self) -> Engine {
        let runtime = self.runtime_builder.build().unwrap();

        Engine::new_with(self.plugins, runtime)
    }
}

/// Struct containing engine config/state,
///
/// # Background
///
/// By definition an engine is a sequence of event. This struct will be built by defining events and sequencing in a seperate file using runmd.
///
/// Events will be configured via a plugin model. Plugins will execute when the event is loaded in the order they are defined.
///
/// Plugins are executed as "Thunks" in a "call-by-name" fashion. Plugins belonging to an event share state linearly, meaning after a plugin executes, it can modify state before the next plugin executes.
///
/// An event may have 1 or more plugins.
///
/// ```md
/// # Example engine definition
///
/// ```runmd <application/lifec.engine> mirror
/// <..start> start
/// <..start> cleanup
/// <..loop>
/// ```
///
/// ```runmd <application/lifec.engine.event> start
/// + .runtime
/// ```
///
/// ```runmd <application/lifec.engine.event> cleanup
/// + .runtime
/// ```
///
/// ```
///
pub struct Engine {
    /// Cancelled when the engine is dropped,
    ///
    pub cancellation: CancellationToken,
    /// Plugins to register w/ the Project
    ///
    plugins: Vec<reality::BlockPlugin<Shared>>,
    /// Wrapped w/ a runtime so that it can be dropped properly
    ///
    runtime: Option<tokio::runtime::Runtime>,
    /// Operations mapped w/ this engine,
    ///
    operations: BTreeMap<String, Operation>,
}

impl Engine {
    /// Creates a new engine builder,
    ///
    pub fn builder() -> EngineBuilder {
        EngineBuilder::new(tokio::runtime::Builder::new_multi_thread())
    }
    
    /// Registers a plugin w/ this engine builder,
    ///
    pub fn register<P: Plugin + Send + Sync + 'static>(&mut self) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<P>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    pub fn register_extension<
        C: ExtensionController<P> + Send + Sync + 'static,
        P: Plugin + Send + Sync + 'static,
    >(
        &mut self,
    ) {
        self.register_with(|parser| {
            parser.with_object_type::<Thunk<ExtensionPlugin<C, P>>>();
        });
    }

    /// Registers a plugin w/ this engine builder,
    ///
    #[inline]
    pub fn register_with(&mut self, plugin: fn(&mut AttributeParser<Shared>)) {
        self.plugins.push(Arc::new(plugin));
    }

    /// Creates a new engine,
    ///
    /// **Note** By default creates a new multi_thread runtime w/ all features enabled
    ///
    pub fn new() -> Self {
        let mut runtime = tokio::runtime::Builder::new_multi_thread();
        runtime.enable_all();

        let runtime = runtime.build().expect("should have an engine");
        Engine::new_with(vec![], runtime)
    }

    /// Creates a new engine w/ runtime,
    ///
    pub fn new_with(
        plugins: Vec<reality::BlockPlugin<Shared>>,
        runtime: tokio::runtime::Runtime,
    ) -> Self {
        Engine {
            plugins,
            runtime: Some(runtime),
            cancellation: CancellationToken::new(),
            operations: BTreeMap::new(),
        }
    }

    /// Creates a new context on this engine,
    ///
    /// **Note** Each time a thunk context is created a new output storage target is generated, however the original storage target is used.
    ///
    pub fn new_context(&self, storage: Arc<tokio::sync::RwLock<Shared>>) -> ThunkContext {
        let mut context = ThunkContext::from(AsyncStorageTarget::from_parts(
            storage,
            self.runtime
                .as_ref()
                .map(|r| r.handle().clone())
                .expect("should have a runtime"),
        ));
        context.cancellation = self.cancellation.child_token();
        context
    }

    /// Compiles operations from the parsed project,
    ///
    pub async fn compile(mut self, workspace: Workspace) -> Self {
        use std::ops::Deref;

        let mut project = Project::new(Shared::default());
        project.add_block_plugin(None, None, |_| {});

        let plugins = self.plugins.clone();
        project.add_node_plugin("operation", move |name, tag, target| {
            let name = name
                .map(|n| n.to_string())
                .unwrap_or(format!("{}", uuid::Uuid::new_v4()));

            if let Some(mut storage) = target.storage_mut() {
                storage.put_resource(Operation::new(name, tag.map(|t| t.to_string())), None)
            }
            
            for p in plugins.iter() {
                p(target);
            }
        });

        if let Some(project) = workspace.compile(project).await.ok().and_then(|mut w| w.project.take()) {
            let nodes = project.nodes.into_inner().unwrap();

            for (_, target) in nodes.iter() {
                if let Some(operation) = target.read().await.resource::<Operation>(None) {
                    let mut operation = operation.deref().clone();
                    operation.bind(self.new_context(target.clone()));
    
                    if let Some(previous) = self.operations.insert(operation.address(), operation) {
                        info!(address=previous.address(), "Replacing operation");
                    }
                }
            }
        }

        self
    }

    /// Runs an operation by address,
    ///
    pub async fn run(&self, address: impl AsRef<str>) -> anyhow::Result<ThunkContext> {
        if let Some(operation) = self.operations.get(address.as_ref()) {
            operation.execute().await
        } else {
            Err(anyhow!("Operation does not exist"))
        }
    }

    /// Returns an iterator over operations,
    ///
    pub fn iter_operations(&self) -> impl Iterator<Item = (&String, &Operation)> {
        self.operations.iter()
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.cancellation.cancel();

        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
