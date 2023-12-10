use anyhow::anyhow;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::OnceLock;

use reality::prelude::*;
use tokio::select;
use tokio_util::sync::CancellationToken;

use crate::host::HostEvent;
use crate::prelude::Action;
use crate::prelude::Address;
use crate::prelude::EngineHandle;
use crate::prelude::Ext;

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
    println!("Creating background work handle");
    let init = Local.create::<BackgroundWork>(tc).await;
    let (_, bh) = tc.maybe_store_kv::<BackgroundWorkEngineHandle>(
        init.address.to_string(),
        BackgroundWorkEngineHandle { tc: tc.clone() },
    );
    let mut _bh = bh.deref().clone();
    drop(bh);

    let engine = tc.engine_handle().await;

    println!("Saving to transient {}", engine.is_some());
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

/// Enumeration of event statuses retrieved by this background work,
///
#[derive(Debug, Clone)]
pub enum EventStatus {
    /// Host event status,
    ///
    Host(HostEvent),
    /// No activity for the event has been seen,
    ///
    None,
}

impl BackgroundWorkEngineHandle {
    /// Calls an plugin by address and returns the current status,
    ///
    pub fn call(
        &mut self,
        address: impl AsRef<str>,
    ) -> anyhow::Result<<Shared as StorageTarget>::BorrowMutResource<'_, BackgroundFuture>> {
        let (_rk, bg) = self.tc.maybe_store_kv(
            address.as_ref(),
            BackgroundFuture {
                tc: self.tc.clone(),
                address: Address::from_str(address.as_ref())?,
                cancellation: self.tc.cancellation.child_token(),
            },
        );

        Ok(bg)
    }

    /// Listens for an event,
    ///
    pub fn listen(&mut self, listen: impl AsRef<str>) -> anyhow::Result<BackgroundFuture> {
        let (_rk, mut bg) = self.tc.maybe_store_kv(
            listen.as_ref(),
            BackgroundFuture {
                tc: self.tc.clone(),
                address: Address::from_str(listen.as_ref())?,
                cancellation: self.tc.cancellation.child_token(),
            },
        );

        Ok(bg.listen())
    }

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
#[derive(Clone)]
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
    /// Spawns the future if enabled otherwise returns the current state,
    ///
    pub fn spawn(&mut self) -> CallStatus {
        if matches!(self.status(), CallStatus::Enabled) {
            if let Some(eh) = self.tc.cached::<EngineHandle>() {
                println!("Spawning from background future {}", self.address);
                let address = self.address.to_string();
                let handle = self.tc.node.runtime.clone().unwrap();

                self.cancellation = self.tc.cancellation.child_token();
                let cancel = self.cancellation.clone();
                let call_output = CallOutput::Spawn(Some(handle.spawn(async move {
                    select! {
                        result = eh.run(address) => result,
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
                println!("Spawning from background future {}", self.address);
                let address = self.address.to_string();
                let handle = self.tc.node.runtime.clone().unwrap();

                self.cancellation = self.tc.cancellation.child_token();
                let cancel = self.cancellation.clone();
                let call_output = CallOutput::Spawn(Some(handle.spawn(async move {
                    let mut resource = eh.hosted_resource(&address).await?;

                    let rk = resource.plugin_rk();
                    unsafe {
                        resource
                            .context_mut()
                            .node_mut()
                            .await
                            .put_resource::<FrameUpdates>(updates, rk.transmute());
                    }

                    select! {
                        result = resource.spawn().unwrap() => result?,
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

    /// Listens to an address that may have spawned in the background,
    ///
    pub fn listen(&mut self) -> BackgroundFuture {
        let address = self.address.clone().with_tag("listen");

        let (_, bg) = self.tc.maybe_store_kv(
            &address.to_string(),
            BackgroundFuture {
                address: address.clone(),
                tc: self.tc.clone(),
                cancellation: self.tc.cancellation.child_token(),
            },
        );

        bg.deref().clone()
    }

    /// Returns the current status of this future,
    ///
    pub fn status(&self) -> CallStatus {
        if let Some((_, calloutput)) = self.tc.fetch_kv::<CallOutput>(&self.address.to_string()) {
            match calloutput.deref() {
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
            }
        } else {
            CallStatus::Enabled
        }
    }

    pub async fn task(&mut self) -> anyhow::Result<ThunkContext> {
        let address = self.address.clone();
        if let Some((_, call)) = self.tc.take_kv::<CallOutput>(&address.to_string()) {
            match call {
                CallOutput::Spawn(Some(spawned)) => select! {
                    result = spawned => {
                        let r = result?;

                        r
                    },
                    _ = self.cancellation.cancelled() => Err(anyhow!("did not spawn task"))
                },
                _ => Err(anyhow!("did not spawn task")),
            }
        } else {
            Err(anyhow!("did not get output"))
        }
    }

    /// Blocks the current thread to surface the background task onto the current thread,
    ///
    /// **Note**: This must be called outside of the tokio-runtime.
    ///
    pub fn into_foreground(&mut self) -> anyhow::Result<ThunkContext> {
        futures::executor::block_on(self.task())
    }

    /// Cancels the current running background future,
    ///
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Returns the current status for an address,
    ///
    pub fn event_status(&mut self) -> EventStatus {
        if let Some((_, event_status)) = self.tc.fetch_kv::<EventStatus>(&self.address.to_string())
        {
            event_status.clone()
        } else {
            EventStatus::None
        }
    }
}

pub struct BackgroundWorker<P>
where
    P: Plugin,
{
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
