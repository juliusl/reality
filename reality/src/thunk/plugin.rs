use std::pin::Pin;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::anyhow;
use tokio::select;
use tokio::sync::Notify;
use tokio::sync::broadcast::error::SendError;
use tokio::sync::watch::Ref;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::trace;
use tracing::warn;

use crate::prelude::*;
use super::prelude::CallAsync;
use super::prelude::CallOutput;
use super::prelude::ThunkContext;

/// Trait to provide a new fn for types that consume plugins,
///
/// Generated when the derive macro is used.
///
pub trait NewFn {
    /// The inner plugin type to create this type from,
    ///
    type Inner;

    /// Returns a new instance from plugin state,
    ///
    fn new(plugin: Self::Inner) -> Self;
}

/// Allows users to export logic as a simple fn,
///
pub trait Plugin: ToFrame + BlockObject<Shared> + CallAsync + Clone + Default {
    /// Associated type of the virtual version of this plugin,
    ///
    /// **Note** If the derive macro is used, this type will be auto-generated w/ the plugin impl,
    ///
    type Virtual: FieldRefController + CallAsync + NewFn + Send + Sync + ToOwned;

    /// Called when an event executes,
    ///
    /// Returning PluginOutput determines the behavior of the Event.
    ///
    fn call(context: ThunkContext) -> CallOutput {
        CallOutput::Spawn(context.spawn(|mut c| async {
            <Self as CallAsync>::call(&mut c).await?;
            Ok(c)
        }))
    }

    /// Enables virtual mode for this plugin,
    ///
    fn enable_virtual(context: ThunkContext) -> CallOutput {
        CallOutput::Spawn(context.spawn(|mut c| async {
            <Self::Virtual as CallAsync>::call(&mut c).await?;
            Ok(c)
        }))
    }

    /// Converts initialized plugin into frame representation and stores
    /// the result to node storage.
    ///
    fn enable_frame(context: ThunkContext) -> CallOutput
    where
        Self::Virtual: NewFn<Inner = Self>,
    {
        CallOutput::Spawn(context.spawn(|c| async {
            eprintln!("enabling frame");
            let init = c.initialized::<Self>().await;
            let frame = init.to_frame(c.attribute);

            let packet_router = PacketRouter::<Self>::new();
            packet_router
                .dispatcher
                .set(c.dispatcher::<FrameUpdates>().await)
                .ok();

            trace!("Create packet routes for resource");
            c.node
                .storage
                .write()
                .await
                .put_resource(std::sync::Arc::new(packet_router), c.attribute.transmute());

            trace!("Putting frame for resource");
            c.node
                .storage
                .write()
                .await
                .put_resource(frame, c.attribute.transmute());

            Ok(c)
        }))
    }

    /// Sync values from context,
    ///
    #[allow(unused_variables)]
    fn sync(&mut self, context: &ThunkContext) {}

    /// Listens for one packet,
    ///
    #[allow(unused_variables)]
    fn listen_one(
        router: std::sync::Arc<PacketRouter<Self>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async { () })
    }
}

pub trait Pack {
    /// Packs the receiver into storage,
    ///
    fn pack<S>(self, storage: &mut S)
    where
        S: StorageTarget;

    /// Unpacks self from Shared,
    ///
    /// The default value for a field will be used if not stored.
    ///
    fn unpack<S>(self, value: &mut S) -> Self
    where
        S: StorageTarget;
}

pub trait FieldRefController {
    /// Sets a field by name to the pending state,
    /// 
    /// Returns true if the field was found and set to pending.
    /// 
    fn set_pending(&mut self, field_name: &str) -> bool;

    /// Returns a list of pending fields,
    /// 
    fn list_pending(&self) -> Vec<&str>;
}

/// Wire server can run in the background and manage sending/receiving frames for a plugn,
///
pub struct WireServer<P: Plugin, const BUFFER_LEN: usize = 1>
where
    P::Virtual: NewFn<Inner = P>,
{
    /// Provides a pipeline for sending and receiving field packets for plugin P,
    ///
    listener: FrameListener<P, BUFFER_LEN>,
    /// Provides a packet router that can be used to handle frames accepted by
    /// the frame listener
    ///
    router: Arc<PacketRouter<P>>,
    /// Cancellation token,
    ///
    cancel: CancellationToken,
    /// Client dispatcher,
    /// 
    dispatcher: OnceLock<Dispatcher<Shared, WireClient<P, BUFFER_LEN>>>,
    /// If the server is running, this notifies a background task that dispatches any pending activity from the client dispatcher,
    /// 
    notify_packet_avail: Arc<Notify>,
}

impl<P, const BUFFER_LEN: usize> WireServer<P, BUFFER_LEN>
where
    P: Plugin,
    P::Virtual: NewFn<Inner = P>,
{
    /// Creates a new wire server for a plugin w/ a thunk context,
    ///
    /// Returns the server that was created.
    ///
    /// **Note** Will enable virtual and frame mode if not already enabled.
    ///
    pub async fn new(tc: &mut ThunkContext) -> anyhow::Result<Arc<WireServer<P, BUFFER_LEN>>> {
        if let Some(init) = P::enable_virtual(tc.clone()).await? {
            if let Some(init) = P::enable_frame(init).await? {
                if let Some(router) = init.router::<P>().await {
                    let plugin = init.initialized::<P>().await;

                    let listener = FrameListener::<P, BUFFER_LEN>::new(plugin);

                    let server = WireServer::<_, BUFFER_LEN> {
                        router,
                        listener,
                        dispatcher: OnceLock::new(),
                        notify_packet_avail: Arc::new(Notify::new()),
                        cancel: tc.cancellation.child_token(),
                    };

                    let server = Arc::new(server);

                    let dispatcher = init.node.maybe_dispatcher(init.attribute.transmute(), server.clone().new_client()).await;

                    server.dispatcher.set(dispatcher).ok();
                    
                    return Ok(server);
                }
            }
        }

        Err(anyhow!("Could not create wire server"))
    }

    /// Starts the wire server w/ one port,
    ///
    pub async fn start(self: Arc<WireServer<P, BUFFER_LEN>>) -> anyhow::Result<()> {
        let mut listener = self.listener.clone();
        let router = self.router.clone();
        let disp = self.dispatcher.get().cloned();
        let cancel = self.cancel.child_token();
        let notify_packet_avail = self.notify_packet_avail.clone();

        // TODO -- allow multiple ports?
        let _port = tokio::spawn(self.start_port());
        
        tokio::spawn(async move {
            loop {
                notify_packet_avail.notified().await;

                if let Some(disp) = disp.as_ref() {
                    let mut disp = disp.clone();
                    disp.dispatch_all().await;
                }
                
                if cancel.is_cancelled() {
                    return;
                }
            }
        });

        while let Ok(next) = listener.listen().await {
            eprintln!("Listener got field: {}", next.field_name);
            if let Err(SendError(pending)) = router.tx.send(next) {
                warn!("Could not route next packet, no receivers are currently listening. Will retry.");

                tokio::time::sleep(Duration::from_millis(100)).await;

                let resend = listener.new_tx().await?;
                resend.send(pending);
            }
        }

        error!("server is exiting");
        _port.abort();

        Ok(())
    }

    /// Creates a new client,
    ///
    pub fn new_client(self: Arc<WireServer<P, BUFFER_LEN>>) -> WireClient<P, BUFFER_LEN> {
        WireClient(self.clone())
    }

    /// Starts a port to listen for changes,
    ///
    async fn start_port(self: Arc<WireServer<P, BUFFER_LEN>>) {
        let listening = self.router.clone();
        let cancel = self.cancel.child_token();

        loop {
            select! {
                _ = P::listen_one(listening.clone()) => {}
                _ = cancel.cancelled() => {
                    debug!("wire server handler is exiting");
                    return;
                }
            }
        }
    }
}

/// Wraps a wire server and provides a client api,
///
#[derive(Clone)]
pub struct WireClient<P, const BUFFER_LEN: usize = 1>(Arc<WireServer<P, BUFFER_LEN>>)
where
    P: Plugin,
    P::Virtual: NewFn<Inner = P>;

impl<P> WireClient<P>
where
    P: Plugin,
    P::Virtual: NewFn<Inner = P>,
{
    /// Queues a modification,
    /// 
    /// This modification will be executed by the client dispatcher therefore an open port must 
    /// be currently listening for the dispatch to be handled. This is different from `try_borrow_modify` which
    /// will try to execute the change immediately.
    /// 
    pub fn queue_modify(
        &self,
        modify: impl FnOnce(Ref<'_, P::Virtual>) -> anyhow::Result<FieldPacket> + Send + Sync + 'static,
    ) {
        if let Some(disp) = self.0.dispatcher.get() {
            disp.clone()
                .queue_dispatch_task(|e| {
                    let client = e.clone();
                    Box::pin(async move {
                        if let Err(err) = client.borrow_and_modify(modify).await {
                            error!("Could not modify upstream plugin {err}");
                        }
                    })
                });

            self.0.notify_packet_avail.notify_one();
        }
    }

    /// If modify returns a packet successfully then this fn will try to send that packet to
    /// the listener. If the packet was successfully sent then Ok(()) is returned.
    /// 
    /// An error is returned in all other cases since the state could have changed when modify was called.
    /// 
    pub fn try_borrow_modify(&self, modify: impl FnOnce(Ref<'_, P::Virtual>) -> anyhow::Result<FieldPacket>) -> anyhow::Result<()> {
        let virt = self.0.listener.borrow_virtual();

        let packet = modify(virt)?;

        self.try_send(packet)?;

        Ok(())
    }

    /// TODO -- This is could be wonky
    /// 
    pub fn try_borrow_modify_batch(&self, modify: impl FnOnce(&mut P::Virtual) -> anyhow::Result<Vec<FieldPacket>>) -> anyhow::Result<()> {
        let mut packets = vec![];
        self.0.listener.update_virtual(|virt| { 
            if let Ok(mut updated) = modify(virt) {
                updated.iter().for_each(|u| {
                    // eprintln!("setting {}", u.field_name);
                    assert!(virt.set_pending(&u.field_name));
                });

                packets.append(&mut updated);
                true
            } else {
                false
            }
        });

        for p in packets {
            self.try_send(p)?;
        }

        Ok(())
    }

    /// 
    /// 
    pub fn try_send(&self, packet: FieldPacket) -> anyhow::Result<()> {
        let tx = self.0.listener.frame_tx();

        let permit = tx.try_reserve()?;
        permit.send(packet);

        Ok(())
    }

    /// Borrows the virtual plugin and if modified transmits a packet to the wire server,
    ///
    /// Returns Ok(()) if a packet was sent successfully.
    ///
    async fn borrow_and_modify(
        &self,
        modify: impl FnOnce(Ref<'_, P::Virtual>) -> anyhow::Result<FieldPacket>,
    ) -> anyhow::Result<()> {
        let v = self.0.listener.borrow_virtual();

        if let Ok(fp) = modify(v) {
            self.send(fp).await?;
        }

        Ok(())
    }

    /// Sends a field packet,
    ///
    async fn send(&self, packet: FieldPacket) -> anyhow::Result<()> {
        let tx = self.0.listener.new_tx().await?;

        tx.send(packet);

        Ok(())
    }
}
