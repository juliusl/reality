use std::pin::Pin;
use std::sync::Arc;
use std::sync::OnceLock;

use futures_util::Stream;
use reality::prelude::*;
use tokio::sync::watch::Receiver;
use tokio::sync::watch::Sender;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::prelude::Action;
use crate::prelude::Address;

use super::Ext;

/// Struct containing a thunk context from a hosted resource node,
///
/// Initializes and enables plugin virtualization inside of the hosted resource's node level storage,
///
/// Plugin virtualization enables communication and coordination between plugins.
///
#[derive(Clone)]
pub struct VirtualBus {
    /// Nodes that can be virtualized by the bus,
    ///
    node: ThunkContext,
    /// Port active notification,
    ///
    port_active: BusPortActive,
}

impl From<ThunkContext> for VirtualBus {
    fn from(node: ThunkContext) -> Self {
        Self {
            node,
            port_active: BusPortActive::default(),
        }
    }
}

impl VirtualBus {
    /// Configures the bus handler to wait for a plugin,
    ///
    pub async fn wait_for<P: Plugin>(
        &mut self,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, BusOwnerPort<P, (), ()>>
    where
        P::Virtual: NewFn<Inner = P> + FieldRefController<Owner = P>,
    {
        if let Some(tc) = self.node.find_node_context::<P>().await {
            if let Ok(Some(context)) = tc.enable_virtual().await {
                let port = self
                    .node
                    .maybe_write_cache::<BusOwnerPort<P, (), ()>>(BusOwnerPort {
                        tx: OnceLock::new(),
                        vrx: OnceLock::new(),
                        sel: |_| panic!("Incomplete bus definition"),
                        port_active: OnceLock::new(),
                        task: OnceLock::new(),
                    });

                if port.tx.get().is_none() {
                    let server = context.wire_server::<P>().await.unwrap();
                    assert!(port
                        .tx
                        .set(
                            server
                                .subscribe_packet_routes()
                                .borrow()
                                .virtual_ref()
                                .send_raw()
                                .clone()
                        )
                        .is_ok());
                }

                if port.vrx.get().is_none() {
                    let server = context.wire_server::<P>().await.unwrap();
                    assert!(port.vrx.set(server.subscribe_packet_routes()).is_ok());
                }

                if port.port_active.get().is_none() {
                    assert!(port.port_active.set(self.port_active.clone()).is_ok());
                }

                if port.task.get().is_none() {
                    let server = context.wire_server::<P>().await.unwrap();

                    assert!(port
                        .task
                        .set(tokio::spawn(async move {
                            info!("Starting wire server and port");
                            tokio::spawn(server.clone().start_port());
                            server.start().await?;
                            Ok(())
                        }))
                        .is_ok());
                }

                return port;
            }
        }

        panic!("Could not find plugin")
    }

    /// Prepares and returns a virtual port on the bus to transmit changes
    /// on the virtual plugin.
    ///
    /// **Note** Waits until a port is active to receive the transmission before sending.
    ///
    pub async fn transmit<P: Plugin>(
        &mut self,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, BusVirtualPort<P>>
    where
        P::Virtual: NewFn<Inner = P> + FieldRefController<Owner = P>,
    {
        if let Some(tc) = self.node.find_node_context::<P>().await {
            if let Ok(Some(context)) = tc.enable_virtual().await {
                debug!("Waiting for port activation before transmission");
                self.port_active.0.notified().await;
                debug!("Port is active, returning port for transmission");
                let port = self
                    .node
                    .maybe_write_cache::<BusVirtualPort<P>>(BusVirtualPort {
                        tx: OnceLock::new(),
                    });

                if port.tx.get().is_none() {
                    let client = context.wire_client::<P>().await.unwrap();
                    assert!(port.tx.set(client.routes()).is_ok());
                }

                return port;
            }
        }

        panic!()
    }
}

#[derive(Default, Clone)]
pub struct BusPortActive(Arc<Notify>);

/// Owner port listening for any published changes to some owner,
///
pub struct BusOwnerPort<Owner: Plugin + 'static, Value: 'static = (), ProjectedValue: 'static = ()>
where
    Owner::Virtual: NewFn<Inner = Owner>,
{
    /// Initialized owner,
    ///
    tx: OnceLock<Arc<Sender<Owner>>>,
    /// Changes to virtual reference,
    ///
    vrx: OnceLock<Receiver<PacketRoutes<Owner>>>,
    /// Selects a field on the owner to receive notifications on,
    ///
    sel: fn(&PacketRoutes<Owner>) -> &FieldRef<Owner, Value, ProjectedValue>,
    /// Notified when StreamExt::next(..) is called,
    ///
    port_active: OnceLock<BusPortActive>,
    /// Server task,
    ///
    task: OnceLock<JoinHandle<anyhow::Result<()>>>,
}

/// Virtual port used to apply changes to the virtual instance receiver (vrx),
///
pub struct BusVirtualPort<Owner: Plugin + 'static> {
    /// Initialized owner,
    ///
    tx: OnceLock<Arc<Sender<PacketRoutes<Owner>>>>,
}

/// Field port that is activated when an field owner has submitted some change,
///
pub struct BusFieldPort<Owner: Plugin + 'static, Value: 'static, ProjectedValue: 'static>
where
    Owner::Virtual: NewFn<Inner = Owner>,
{
    /// Owner bus,
    ///
    owner_port: BusOwnerPort<Owner, Value, ProjectedValue>,
    /// Filter predicate that must return true before this port can be activated,
    ///
    filter: fn(&FieldRef<Owner, Value, ProjectedValue>) -> bool,
}

impl<Owner: Plugin + 'static> BusVirtualPort<Owner> {
    /// Writes to the virtual port,
    ///
    pub fn write_to_virtual(&self, update: impl FnOnce(&mut PacketRoutes<Owner>) -> bool) {
        if let Some(tx) = self.tx.get() {
            if tx.send_if_modified(|virt| update(virt)) {
                debug!("updating");
            } else {
                debug!("skip updating");
            }
        } else {
            panic!()
        }
    }
}

impl<Owner: Plugin + 'static> BusOwnerPort<Owner, (), ()>
where
    Owner::Virtual: NewFn<Inner = Owner>,
{
    /// Selects a field to monitor changes on from the owner,
    ///
    pub fn select<Value: 'static, ProjectedValue: 'static>(
        &self,
        sel: fn(&PacketRoutes<Owner>) -> &FieldRef<Owner, Value, ProjectedValue>,
    ) -> BusFieldPort<Owner, Value, ProjectedValue> {
        BusFieldPort {
            owner_port: BusOwnerPort {
                sel,
                tx: self.tx.clone(),
                vrx: self.vrx.clone(),
                port_active: self.port_active.clone(),
                task: OnceLock::new(),
            },
            filter: |_| true,
        }
    }

    /// Subscribes to all owner changes w/o any field refs,
    ///
    /// **Panics** Will panic if the tx is not set, which means that this type was created manually instead
    /// of through the OwnerPort.
    ///
    pub fn subscribe_raw(&self) -> tokio::sync::watch::Receiver<Owner> {
        let tx = self.tx.get().expect("should be set");
        tx.subscribe()
    }
}

impl<Owner: Plugin + 'static, Value: 'static, ProjectedValue: 'static>
    BusFieldPort<Owner, Value, ProjectedValue>
where
    Owner::Virtual: NewFn<Inner = Owner>,
{
    /// Applies a filter on a received field,
    ///
    pub fn filter(mut self, filter: fn(&FieldRef<Owner, Value, ProjectedValue>) -> bool) -> Self {
        self.filter = filter;
        self
    }

    /// Returns a pinned port,
    ///
    /// **Usage**
    ///
    /// ```rs no_run
    /// let mut stream = port.pinned();
    ///
    /// while let Some(next) = stream.deref_mut().next().await {
    /// ..
    /// }
    /// ```
    ///
    pub fn pinned(self) -> Pin<Box<Self>> {
        Box::pin(self)
    }
}

impl<'a, V, Owner, Value, ProjectedValue> Stream
    for &'a mut BusFieldPort<Owner, Value, ProjectedValue>
where
    V: CallAsync
        + ToOwned<Owned = Owner>
        + NewFn<Inner = Owner>
        + FieldRefController<Owner = Owner>
        + 'a,
    Owner: Plugin<Virtual = V> + 'static,
    Value: 'static,
    ProjectedValue: 'static,
{
    type Item = (FieldRef<Owner, Value, ProjectedValue>, Owner);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let filter = self.filter;
        let sel = self.owner_port.sel;
        let tx = self.owner_port.tx.clone();

        if let Some(port_active) = self.owner_port.port_active.get() {
            port_active.0.notify_waiters();
        } else {
            warn!("No active port set");
        }

        if let Some(rx) = self.owner_port.vrx.get_mut() {
            match rx.has_changed() {
                Ok(true) => {
                    let next = rx.borrow_and_update();

                    let virt = next;
                    let field = sel(&virt);

                    let next = virt.virtual_ref().current();

                    if filter(field) {
                        // If field ref is in the committed state, notify any raw listeners
                        if field.is_committed() {
                            debug!("Field committed, notifying listeners of owner actual");
                            if let Some(tx) = tx.get() {
                                // Check if any "raw" listeners are listening before sending update
                                if !tx.is_closed() {
                                    // If "raw" listeners are listening, update them when fields are committed
                                    if let Err(err) = tx.send(next.clone()) {
                                        error!("{err}");
                                    }
                                }
                            } else {
                                panic!("tx not set")
                            }
                        }

                        std::task::Poll::Ready(Some((field.clone(), next)))
                    } else {
                        cx.waker().wake_by_ref();
                        std::task::Poll::Pending
                    }
                }
                Ok(false) => {
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
                Err(err) => {
                    error!("{err}");
                    std::task::Poll::Ready(None)
                }
            }
        } else {
            eprintln!("stream is exiting");
            std::task::Poll::Ready(None)
        }
    }
}

#[async_trait]
pub trait VirtualBusExt: AsRef<ThunkContext> + AsMut<ThunkContext> {
    /// Returns a virtual bus a hosted resource,
    ///
    /// If the hosted resource cannot be found, returns a virtual bus for the
    /// current context.
    ///
    async fn virtual_bus(&self, address: impl Into<Address> + Send) -> VirtualBus
    where
        Self: Sync,
    {
        let address = address.into();

        if let Some(eh) = self.as_ref().engine_handle().await {
            debug!("Finding host");
            if let Ok(r) = eh.hosted_resource(address.to_string()).await {
                debug!("creating new bus");
                VirtualBus {
                    node: r.context().clone(),
                    port_active: BusPortActive::default(),
                }
            } else {
                warn!("Could not find hosted resource at {address}");
                VirtualBus {
                    node: self.as_ref().clone(),
                    port_active: BusPortActive::default(),
                }
            }
        } else {
            warn!("Could not find hosted resource at {address}");
            VirtualBus {
                node: self.as_ref().clone(),
                port_active: BusPortActive::default(),
            }
        }
    }
}

impl VirtualBusExt for ThunkContext {}
