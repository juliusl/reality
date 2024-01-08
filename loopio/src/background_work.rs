use anyhow::anyhow;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::OnceLock;
use tracing::debug;

use reality::prelude::*;
use tokio::select;
use tokio_util::sync::CancellationToken;

use crate::prelude::Action;
use crate::prelude::Address;
use crate::prelude::EngineHandle;
use crate::prelude::Ext;
use crate::work::PrivateProgress;
use crate::work::PrivateStatus;
use crate::work::WorkState;
use crate::work::__WorkState;

/// Background work container,
///
/// **Notes**
/// - Start jobs that can be monitored
/// - Get jobs by name
/// - Basic api's
///
#[derive(Reality, Default, Clone)]
#[reality(call = create_background_work_handle, plugin, rename = "background-work")]
pub struct BackgroundWork {
    /// Addressable background work,
    ///
    #[reality(derive_fromstr)]
    address: Address,
}

/// Creates a new background work resource,
///
async fn create_background_work_handle(tc: &mut ThunkContext) -> anyhow::Result<()> {
    debug!("Creating background work handle");
    let init = Local.create::<BackgroundWork>(tc).await;
    let (_, bh) = tc.maybe_store_kv::<BackgroundWorkEngineHandle>(
        init.address.to_string(),
        BackgroundWorkEngineHandle { tc: tc.clone() },
    );
    let mut _bh = bh.deref().clone();
    drop(bh);

    let engine = tc.engine_handle().await;

    debug!("Saving to transient {}", engine.is_some());
    _bh.tc
        .write_cache(engine.expect("should be bound to an engine"));
    tc.transient_mut()
        .await
        .put_resource(_bh, ResourceKey::root());
    Ok(())
}

/// Manages background work in contexts when a foreground/background thread are present,
///
#[derive(Clone)]
pub struct BackgroundWorkEngineHandle {
    pub tc: ThunkContext,
}

/// Enumeration of call statuses by this background work handle,
///
#[derive(Debug)]
pub enum CallStatus {
    /// Whether this call is enabled (found but not running),
    ///
    Enabled,
    /// Whether this call is disabled (not found),
    ///
    Disabled,
    /// Whether this call is running (found and running),
    ///
    Running,
    /// Whether this call is pending output handling,
    ///
    Pending,
}

impl BackgroundWorkEngineHandle {
    /// Returns a new background future for address hosted by engine,
    ///
    pub fn call(
        &mut self,
        address: impl AsRef<str>,
    ) -> anyhow::Result<<Shared as StorageTarget>::BorrowMutResource<'_, BackgroundFuture>> {
        let (_rk, mut bg) = self.tc.maybe_store_kv(
            address.as_ref(),
            BackgroundFuture {
                tc: self.tc.clone(),
                address: Address::from_str(address.as_ref())?,
                cancellation: self.tc.cancellation.child_token(),
            },
        );

        bg.enable_work_state();

        Ok(bg)
    }

    /// Creates a new background worker which implements tower::Service,
    ///  
    pub fn worker<P>(
        &mut self,
        plugin: P,
    ) -> anyhow::Result<<Shared as StorageTarget>::BorrowMutResource<'_, BackgroundWorker<P>>>
    where
        P: Plugin,
    {
        let (_rk, bg) = self.tc.maybe_store_kv(
            P::symbol(),
            BackgroundWorker {
                inner: OnceLock::new(),
            },
        );

        bg.inner
            .set(plugin)
            .map_err(|_| anyhow!("existing plugin has not been handled yet"))?;

        Ok(bg)
    }
}

/// API for managing background tasks,
///
// #[derive(Clone)] This can be tricky if the context is cloned since the cache will also be cloned, therefore the call-output wouldn't be able to returned
pub struct BackgroundFuture {
    /// Address of the background future,
    ///
    address: Address,
    /// Thunk context,
    ///
    tc: ThunkContext,
    /// Cancellation token,
    ///
    cancellation: CancellationToken,
}

impl AsRef<ThunkContext> for BackgroundFuture {
    fn as_ref(&self) -> &ThunkContext {
        &self.tc
    }
}

impl AsMut<ThunkContext> for BackgroundFuture {
    fn as_mut(&mut self) -> &mut ThunkContext {
        &mut self.tc
    }
}

impl From<&ThunkContext> for BackgroundFuture {
    fn from(value: &ThunkContext) -> Self {
        BackgroundFuture {
            address: Address::default(),
            tc: value.clone(),
            cancellation: value.cancellation.child_token(),
        }
    }
}

impl BackgroundFuture {
    /// Prepares the work state for the background future,
    ///
    pub fn enable_work_state(&mut self) {
        self.work_state().init();

        let init = { self.tc.virtual_work_state_mut(None) };
        let progress = init.progress.clone();
        let status = init.status.clone();
        drop(init);
        self.tc.write_cache::<PrivateProgress>(progress);
        self.tc.write_cache::<PrivateStatus>(status);
    }

    /// Spawns the future if enabled otherwise returns the current state,
    ///
    pub fn spawn(&mut self) -> CallStatus {
        if matches!(self.status(), CallStatus::Enabled) {
            if let Some(eh) = self.tc.cached::<EngineHandle>() {
                // TODO: Check work state?

                debug!("Spawning from background future {}", self.address);
                let address = self.address.to_string();
                let handle = self.tc.node.runtime.clone().unwrap();

                self.cancellation = self.tc.cancellation.child_token();
                let cancel = self.cancellation.clone();

                // Prepare to run the plugin w/ this context
                let mut context = self.tc.clone();
                let call_output = CallOutput::Spawn(Some(handle.spawn(async move {
                    select! {
                        resource = eh.hosted_resource(address) => {
                            // Rebuild the environment for the current context
                            let resource = resource?;
                            context.node = resource.context().node.clone();
                            context.attribute = resource.context().attribute;
                            if let Some(plugin) = resource.context().attribute.plugin().and_then(|p| p.call()) {
                               Ok(plugin(context).await?.unwrap())
                            } else {
                                Err(anyhow!("Resource is missing plugin implementation"))
                            }
                        },
                        _ = cancel.cancelled() => {
                            Err(anyhow!("Call was cancelled"))
                        }
                    }
                })));

                self.tc.store_kv(&self.address.to_string(), call_output);

                CallStatus::Running
            } else {
                CallStatus::Disabled
            }
        } else {
            self.status()
        }
    }

    /// Spawn the hosted resource w/ frame updates,
    ///
    pub fn spawn_with_updates(&mut self, updates: FrameUpdates) -> CallStatus {
        if matches!(self.status(), CallStatus::Enabled) {
            if let Some(eh) = self.tc.cached::<EngineHandle>() {
                debug!("Spawning from background future {}", self.address);
                let address = self.address.to_string();
                let handle = self.tc.node.runtime.clone().unwrap();

                self.cancellation = self.tc.cancellation.child_token();
                let cancel = self.cancellation.clone();
                let mut context = self.tc.clone();
                let call_output = CallOutput::Spawn(Some(handle.spawn(async move {
                    select! {
                        resource = eh.hosted_resource(address) => {
                            // Rebuild the environment for the current context
                            let resource = resource?;
                            let rk = resource.plugin_rk();
                            context
                                .node()
                                .await
                                .lazy_put_resource::<FrameUpdates>(updates, rk.transmute());
                            context.process_node_updates().await;
                            context.node = resource.context().node.clone();
                            context.attribute = resource.context().attribute;
                            if let Some(plugin) = resource.context().attribute.plugin().and_then(|p| p.call()) {
                               Ok(plugin(context).await?.unwrap())
                            } else {
                                Err(anyhow!("Resource is missing plugin implementation"))
                            }
                        },
                        _ = cancel.cancelled() => {
                            Err(anyhow!("Call was cancelled"))
                        }
                    }
                })));

                self.tc.store_kv(&self.address.to_string(), call_output);
                CallStatus::Running
            } else {
                CallStatus::Disabled
            }
        } else {
            self.status()
        }
    }

    /// Returns the current status of this future,
    ///
    pub fn status(&self) -> CallStatus {
        if let Some((_, calloutput)) = self.tc.fetch_kv::<CallOutput>(&self.address.to_string()) {
            let calloutput = match calloutput.deref() {
                CallOutput::Spawn(spawned) => match spawned {
                    Some(jh) => {
                        if jh.is_finished() {
                            CallStatus::Pending
                        } else {
                            CallStatus::Running
                        }
                    }
                    None => CallStatus::Pending,
                },
                CallOutput::Abort(_) => CallStatus::Pending,
                CallOutput::Skip => CallStatus::Disabled,
                CallOutput::Update(_) => CallStatus::Pending,
            };
            calloutput
        } else {
            CallStatus::Enabled
        }
    }

    /// Converts the current background future into an actual future,
    ///
    /// **Error** Returns an error if the task was not previously spawned, or if
    /// the running task could not be removed from the cache
    ///
    pub async fn task(&mut self) -> anyhow::Result<ThunkContext> {
        if let Some((_, call)) = self.tc.take_kv::<CallOutput>(&self.address.to_string()) {
            match call {
                CallOutput::Spawn(Some(spawned)) => select! {
                    result = spawned => {
                        self.work_state().set_work_stop();
                        let r = result?;
                        r
                    },
                    _ = self.cancellation.cancelled() => Err(anyhow!("did not spawn task"))
                },
                _ => Err(anyhow!("Did not spawn task")),
            }
        } else {
            Err(anyhow!("Did not get output"))
        }
    }

    /// Blocks the current thread to surface the background task onto the current thread,
    ///
    /// **Panic**: This must be called outside of the tokio-runtime or it will result in a panic.
    ///
    pub fn into_foreground(&mut self) -> anyhow::Result<ThunkContext> {
        futures::executor::block_on(self.task())
    }

    /// Cancels the current running background future,
    ///
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Returns work state for configuring the current work state,
    ///
    pub fn work_state(&mut self) -> &mut impl WorkState {
        &mut self.tc
    }

    /// Blocks the current thread to wait for the background future to complete,
    ///
    /// **Error** Returns an error if the background future is ready but returned an error.
    ///
    pub fn wait_for_completion<T: BackgroundFutureController>(
        &mut self,
        controller: &mut T,
    ) -> anyhow::Result<ThunkContext> {
        loop {
            match self.tick(controller) {
                Ok(tc) => {
                    break Ok(tc);
                }
                Err(err) if err.to_string().as_str() == "still running" => {
                    continue;
                }
                Err(err) => {
                    break Err(err);
                }
            }
        }
    }

    /// Ticks the background future for completion,
    ///
    /// **Error** Returns an error if the background future is still running or if the background future is ready but returned an error.
    ///
    pub fn tick<T: BackgroundFutureController>(
        &mut self,
        controller: &mut T,
    ) -> anyhow::Result<ThunkContext> {
        match self.inner_poll_ready(controller) {
            std::task::Poll::Ready(ready) => Ok(ready?),
            std::task::Poll::Pending => Err(anyhow!("still running")),
        }
    }

    /// Inner polling function,
    ///
    pub(crate) fn inner_poll_ready<T: BackgroundFutureController>(
        &mut self,
        controller: &mut T,
    ) -> std::task::Poll<anyhow::Result<ThunkContext>> {
        match self.status() {
            CallStatus::Enabled => match controller.on_enabled(self) {
                std::task::Poll::Ready(Err(err)) => std::task::Poll::Ready(Err(err)),
                _ => std::task::Poll::Pending,
            },
            CallStatus::Disabled => match controller.on_disabled(self) {
                std::task::Poll::Ready(Err(err)) => std::task::Poll::Ready(Err(err)),
                _ => std::task::Poll::Pending,
            },
            CallStatus::Running => match controller.on_running(self) {
                std::task::Poll::Ready(Err(err)) => std::task::Poll::Ready(Err(err)),
                _ => std::task::Poll::Pending,
            },
            CallStatus::Pending => controller.on_pending(self),
        }
    }
}

/// Trait for controlling the behavior of a background future,
///
pub trait BackgroundFutureController {
    /// Called when the background future should be enabled,
    ///
    fn on_enabled(&mut self, bg: &mut BackgroundFuture) -> std::task::Poll<anyhow::Result<()>> {
        bg.spawn();
        std::task::Poll::Pending
    }

    /// Called when the background future should be disabled,
    ///
    fn on_disabled(&mut self, _: &mut BackgroundFuture) -> std::task::Poll<anyhow::Result<()>> {
        std::task::Poll::Pending
    }

    /// Called when the current background future is running in the background,
    ///
    fn on_running(&mut self, _: &mut BackgroundFuture) -> std::task::Poll<anyhow::Result<()>> {
        std::task::Poll::Pending
    }

    /// Called when the current status is pending,
    ///
    fn on_pending(
        &mut self,
        bg: &mut BackgroundFuture,
    ) -> std::task::Poll<anyhow::Result<ThunkContext>> {
        std::task::Poll::Ready(bg.into_foreground())
    }
}

/// Default background future controller,
///
/// **Note** The default behavior is to spawn the background task immediately if enabled. No-op on disabled.
///
pub struct DefaultController;

impl BackgroundFutureController for DefaultController {}

/// Tower wrapper for plugins,
///
pub struct BackgroundWorker<P>
where
    P: Plugin,
{
    /// Inner plugin state,
    ///
    /// When set the service will be ready and will return a future containing
    /// the result of calling the inner plugin
    ///
    inner: OnceLock<P>,
}

impl<P> tower::Service<ThunkContext> for BackgroundWorker<P>
where
    P: Plugin,
{
    type Response = ThunkContext;

    type Error = anyhow::Error;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        if self.inner.get().is_some() {
            std::task::Poll::Ready(Ok(()))
        } else {
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    }

    fn call(&mut self, mut context: ThunkContext) -> Self::Future {
        let inner = self
            .inner
            .take()
            .expect("should be initialized if being called");

        Box::pin(async move {
            let mut s = context.node.storage.write().await;
            s.put_resource(inner, context.attribute.transmute());
            drop(s);

            <P as CallAsync>::call(&mut context).await?;
            Ok(context)
        })
    }
}
