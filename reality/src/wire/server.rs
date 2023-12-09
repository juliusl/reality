use std::{sync::Arc, time::Duration};

use reality_derive::Reality;
use tokio::{select, sync::{broadcast::error::SendError, watch::Ref}};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use crate::prelude::*;

use anyhow::anyhow;

pub trait FieldRefController {
    /// Field owner type,
    /// 
    type Owner: Plugin;

    /// Sets a field by name to the pending state,
    ///
    /// Returns true if the field was found and set to pending.
    ///
    fn set_pending(&mut self, field_name: &str) -> bool;

    /// Returns a list of pending fields,
    ///
    fn list_pending(&self) -> Vec<&str>;

    /// Returns a channel receiver to watch for changes to the field ref owner,
    /// 
    /// **Note** This channel is noisy and will not receive any information on which fields changed. In addition it may be notified
    /// even if no changes actually occured.
    /// 
    fn listen_raw(&self) -> tokio::sync::watch::Receiver<Self::Owner>;

    /// Returns a reference to the tx side of the watch channel over Owner,
    /// 
    fn send_raw(&self) -> Arc<tokio::sync::watch::Sender<Self::Owner>>;

    /// Returns the current state of owner,
    /// 
    fn current(&self) -> Self::Owner;
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
        if let Some(init) = P::enable_frame(tc.clone()).await? {
            if let (Some(router), Some(listener)) =
                (init.router::<P>().await, init.listener::<P>().await)
            {
                let server = WireServer::<_, BUFFER_LEN> {
                    router,
                    listener: listener.with_buffer_size(),
                    cancel: tc.cancellation.child_token(),
                };

                let server = Arc::new(server);

                // TODO -- eventually this could be useful
                // let dispatcher = init.node.maybe_dispatcher(init.attribute.transmute(), server.clone().new_client()).await;
                // server.dispatcher.set(dispatcher).ok();

                return Ok(server);
            }
        }

        Err(anyhow!("Could not create wire server"))
    }

    /// Starts the wire server w/ one port,
    ///
    pub async fn start(self: Arc<WireServer<P, BUFFER_LEN>>) -> anyhow::Result<()> {
        let mut listener = self.listener.clone();
        let router = self.router.clone();
        let cancel = self.cancel.child_token();

        // TODO -- Currently only one port starts to route changes
        let _port = tokio::spawn(self.start_port());

        // TODO -- if a client dispatcher is in-use this is required to handle new changes
        // tokio::spawn(async move {
        //     loop {
        //         notify_packet_avail.notified().await;

        //         if let Some(disp) = disp.as_ref() {
        //             let mut disp = disp.clone();
        //             disp.dispatch_all().await;
        //         }

        //         if cancel.is_cancelled() {
        //             return;
        //         }
        //     }
        // });

        while let Ok(next) = select! {
            next = listener.listen() => next,
            _ = cancel.cancelled() => {
                return Err(anyhow!("Process is shutting down down"))
            }
        } {
            // TODO -- fix the ordering of this
            for n in next {
                debug!("Listener got field: {}", n.field_name);
                if let Err(SendError(pending)) = router.tx.send(n) {
                    debug!("Could not route next packet, no receivers are currently listening. Will retry.");

                    tokio::time::sleep(Duration::from_millis(100)).await;
                    // TODO -- This can be an issue since it would reorder the packets in an unexpected way if packets were sent in batch
                    let resend = listener.new_tx().await?;
                    resend.send(vec![pending]);
                } else {
                    debug!("Sent update to router");
                }
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

    /// Subscribe to changes to packet routes,
    ///
    pub fn subscribe_packet_routes(
        self: Arc<WireServer<P, BUFFER_LEN>>,
    ) -> tokio::sync::watch::Receiver<PacketRoutes<P>> {
        self.listener.subscribe_virtual()
    }

    /// Starts a port to listen for changes,
    ///
    /// **Note**: This is where packets sent from router.tx are handled.
    ///
    /// The packet is applied to all routes once and if successfully applied it is sent to the frame updates
    /// dispatcher.
    ///
    pub async fn start_port(self: Arc<WireServer<P, BUFFER_LEN>>) {
        let listening = self.router.clone();
        let cancel = self.cancel.child_token();

        loop {
            select! {
                _ = P::listen_one(listening.clone()) => {
                    self.router.routes.send_if_modified(|_r| {
                        true
                    });
                }
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
    P::Virtual: NewFn<Inner = P> + FieldRefController,
{
    /// If modify returns a packet successfully then this fn will try to send that packet to
    /// the listener. If the packet was successfully sent then Ok(()) is returned.
    ///
    /// An error is returned in all other cases since the state could have changed when modify was called.
    ///
    pub fn try_borrow_modify(
        &self,
        modify: impl FnOnce(Ref<'_, PacketRoutes<P>>) -> anyhow::Result<FieldPacket>,
    ) -> anyhow::Result<()> {
        let virt = self.0.listener.routes();

        let packet = modify(virt.borrow())?;

        self.try_send(vec![packet])?;

        Ok(())
    }

    /// Send a batch of field packets at once,
    ///
    pub fn try_borrow_modify_batch(
        &self,
        modify: impl FnOnce(Ref<'_, PacketRoutes<P>>) -> anyhow::Result<Vec<FieldPacket>>,
    ) -> anyhow::Result<()> {
        let updates = modify(self.0.listener.routes().borrow())?;

        self.try_send(updates)?;

        Ok(())
    }

    /// Tries to send a batch of field packets to the frame listener,
    ///
    pub fn try_send(&self, packets: Vec<FieldPacket>) -> anyhow::Result<()> {
        let tx = self.0.listener.frame_tx();

        let permit = tx.try_reserve()?;
        permit.send(packets);

        Ok(())
    }

    /// Subscribe to changes on the inner packet routes,
    /// 
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<PacketRoutes<P>> {
        self.0.clone().subscribe_packet_routes()
    }

    /// Returns a reference to a routes updater,
    /// 
    pub fn routes(&self) -> Arc<tokio::sync::watch::Sender<PacketRoutes<P>>> {
        self.0.listener.routes()
    }

    // /// Queues a modification,
    // ///
    // /// This modification will be executed by the client dispatcher therefore an open port must
    // /// be currently listening for the dispatch to be handled. This is different from `try_borrow_modify` which
    // /// will try to execute the change immediately.
    // ///
    // pub fn queue_modify(
    //     &self,
    //     modify: impl FnOnce(Ref<'_, PacketRoutes<P>>) -> anyhow::Result<FieldPacket> + Send + Sync + 'static,
    // ) {
    //     if let Some(disp) = self.0.dispatcher.get() {
    //         disp.clone()
    //             .queue_dispatch_task(|e| {
    //                 let client = e.clone();
    //                 Box::pin(async move {
    //                     if let Err(err) = client.borrow_and_modify(modify).await {
    //                         error!("Could not modify upstream plugin {err}");
    //                     }
    //                 })
    //             });
    //         self.0.notify_packet_avail.notify_one();
    //     }
    // }
}

pub async fn enable_virtual_dependencies<P: Plugin>(tc: &mut ThunkContext) -> anyhow::Result<()>
where
    P::Virtual: NewFn<Inner = P>,
{
    // Enable wire server
    debug!("Enabling wire server");
    let wire_server = WireServer::<P>::new(tc).await?;

    let mut storage = tc.node.storage.write().await;
    storage.maybe_put_resource(wire_server.clone(), tc.attribute.transmute());
    storage.maybe_put_resource(wire_server.new_client(), tc.attribute.transmute());

    Ok(())
}

#[derive(Reality, Debug, Default, Clone)]
#[reality(call = test, plugin)]
pub struct Test {
    #[reality(derive_fromstr)]
    name: String,
}

async fn test(_: &mut ThunkContext) -> anyhow::Result<()> {
    Ok(())
}

#[tokio::test]
#[tracing_test::traced_test]
async fn test_wire_server() {
    let mut tc = ThunkContext::new();

    let server = WireServer::<Test>::new(&mut tc).await.unwrap();

    let running = tokio::spawn(server.clone().start());

    let ct = server.cancel.child_token();

    let mut listen_routes = server.clone().subscribe_packet_routes();

    let _listen_routes = tokio::spawn(async move {
        while !ct.is_cancelled() {
            if let Ok(()) = listen_routes.changed().await {
                let _next = listen_routes.borrow_and_update();
                _next.route::<0>().view_value(|v| {
                    assert_eq!(v, "hello town");
                    eprintln!("got change: {v}");
                });

                return;
            }
        }
    });

    let client = server.clone().new_client();

    client
        .try_borrow_modify(|_r| {
            let fields = VisitVirtual::<String, String>::visit_fields(&_r);
            for field in fields {
                field.edit_value(|_, n| {
                    *n = String::from("hello town");
                    true
                });
            }

            let field = _r.route::<0>();
            Ok(field.encode())
        })
        .unwrap();

    // Test route change notification
    _listen_routes.await.unwrap();

    // Test shutdown
    server.cancel.cancel();

    running.await.unwrap().expect_err("should be canceled");
    ()
}
