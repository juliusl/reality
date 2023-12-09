use std::fmt::Debug;
use std::sync::Arc;
use std::sync::OnceLock;

use anyhow::anyhow;
use futures_util::Stream;
use reality::prelude::*;
use reality::WireClient;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::watch::Receiver;
use tokio::sync::watch::Sender;
use tokio::task::JoinHandle;
use tracing::error;
use tracing::info;

use crate::prelude::Action;
use crate::prelude::Address;

use super::Ext;

/// Converts the type being extended into wire format,
///
/// Middleware can be configured on the bus to operate on the frame before applying it.
///
#[derive(Default, Debug, Clone)]
pub struct WireBus {
    /// Current frame,
    ///
    frame: Frame,
}

impl WireBus {
    /// Returns a vector of packets currently stored on the bus,
    ///
    pub fn packets(&self) -> Vec<FieldPacket> {
        // TODO: This could be optimized later, but for brevity this is what needs to be returned,
        [self.frame.recv.clone()]
            .iter()
            .chain(self.frame.fields.iter())
            .cloned()
            .collect::<Vec<FieldPacket>>()
    }
}

/// Plugin to enable the wire bus on an attribute,
///
#[derive(Reality, Serialize, Deserialize, PartialEq, Default, Clone)]
#[reality(call=enable_wire_bus, plugin, rename = "enable-wirebus")]
pub struct EnableWireBus {
    /// Path to the attribute,
    ///
    /// **Note**: A path must be assigned to an attribute in order for it to be
    /// navigated to by this plugin.
    ///
    #[reality(derive_fromstr)]
    path: String,
    /// If true allows changes to be applied,
    ///
    allow_frame_updates: bool,
}

async fn enable_wire_bus(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<EnableWireBus>().await;

    if let Some(mut path) = tc.navigate(&init.path).await {
        info!("Enabling wire bus {}", init.path);
        if let Some(enabled) = path.context().enable_frame().await? {
            let attr = path.context().attribute.clone();
            let frame = enabled.initialized_frame().await;
            unsafe {
                // Creates a new wire bus
                path.context_mut()
                    .node_mut()
                    .await
                    .put_resource(WireBus { frame }, attr.transmute());

                // If enabled this will enable frame updates for the plugin,
                if init.allow_frame_updates {
                    path.context_mut()
                        .node_mut()
                        .await
                        .maybe_put_resource::<FrameUpdates>(
                            FrameUpdates::default(),
                            attr.transmute(),
                        );
                }
            }
        }
        Ok(())
    } else {
        Err(anyhow!("Could not find resource {:?}", init.path))
    }
}

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
}

/// Initializes and enables a virtual context for the **last** instance of the plugin P found,
///
macro_rules! init_virtual_context {
    ($rcv:ident, $r:path) => {{
        let c = OnceLock::new();
        if let Some(tc) = $rcv.node.find_node_context::<P>().await {
            if let Ok(Some(context)) = tc.enable_virtual().await {
                if let Some(_c) = context
                    .node()
                    .await
                    .current_resource::<$r>(context.attribute.transmute())
                {
                    assert!(c.set(_c).is_ok())
                }
            }
        }
        c
    }};
}

impl VirtualBus {
    /// Configures the bus handler to wait for a plugin,
    ///
    pub async fn wait_for<P: Plugin>(
        &mut self,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, BusOwnerPort<P, (), ()>>
    where
        P::Virtual: NewFn<Inner = P>,
    {
        if let Some(tc) = self.node.find_node_context::<P>().await {
            if let Ok(Some(context)) = tc.enable_virtual().await {
                let wire_server = context.wire_server::<P>().await.unwrap();

                let client = wire_server.clone().new_client();

                let rx = self
                    .node
                    .maybe_write_cache::<BusOwnerPort<P, (), ()>>(BusOwnerPort {
                        client: client.clone(),
                        vrx: {
                            let lock = OnceLock::new();
                            assert!(lock.set(client.subscribe()).is_ok());
                            lock
                        },
                        sel: |_| panic!("Incomplete bus definition"),
                        _port: {
                            let lock = OnceLock::new();
                            lock.set(tokio::spawn(wire_server.clone().start_port()))
                                .expect("should be empty");
                            lock
                        },
                    });

                return rx;
            } else {
                error!("Could not enable virtual mode for plugin");
            }
        } else {
            error!("Could not find node context");
        }

        panic!("Could not find plugin")
    }

    /// Prepares and returns a virtual port on the bus to transmit changes
    /// on the virtual plugin.
    ///
    pub async fn transmit<P: Plugin>(
        &mut self,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, BusVirtualPort<P>> {
        let rx = self
            .node
            .maybe_write_cache::<BusVirtualPort<P>>(BusVirtualPort {
                tx: init_virtual_context!(self, Arc<tokio::sync::watch::Sender<P::Virtual>>),
            });

        rx
    }
}

/// Owner port listening for any published changes to some owner,
///
pub struct BusOwnerPort<Owner: Plugin + 'static, Value: 'static = (), ProjectedValue: 'static = ()>
where
    Owner::Virtual: NewFn<Inner = Owner>,
{
    /// Wire client,
    ///
    client: WireClient<Owner>,
    /// Receiver for updates to any of Owner's packet routes,
    ///
    vrx: OnceLock<Receiver<PacketRoutes<Owner>>>,
    /// Running wire-server port listening for changes from the wire server,
    ///
    _port: OnceLock<JoinHandle<()>>,
    /// Selects a field on the owner to receive notifications on,
    ///
    sel: fn(&Owner::Virtual) -> &FieldRef<Owner, Value, ProjectedValue>,
}

/// Virtual port used to apply changes to the virtual instance receiver (vrx),
///
pub struct BusVirtualPort<Owner: Plugin + 'static> {
    /// Initialized owner,
    ///
    tx: OnceLock<Arc<Sender<Owner::Virtual>>>,
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
    pub fn write_to_virtual(&self, update: impl FnOnce(&mut Owner::Virtual) -> bool) {
        if let Some(tx) = self.tx.get() {
            if tx.send_if_modified(|virt| update(virt)) {}
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
        sel: fn(&Owner::Virtual) -> &FieldRef<Owner, Value, ProjectedValue>,
    ) -> BusFieldPort<Owner, Value, ProjectedValue> {
        BusFieldPort {
            owner_port: BusOwnerPort {
                sel,
                vrx: self.vrx.clone(),
                client: self.client.clone(),
                _port: OnceLock::new(), // Don't actually need an initialized port so can just create an empty lock here
            },
            filter: |_| true,
        }
    }

    // /// Subscribes to all owner changes w/o any field refs,
    // ///
    // /// **Panics** Will panic if the tx is not set, which means that this type was created manually instead
    // /// of through the OwnerPort.
    // ///
    // pub fn subscribe_raw(&self) -> tokio::sync::watch::Receiver<Owner> {
    //     let tx = self.tx.get().expect("should be set");
    //     tx.subscribe()
    // }
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
}

impl<'a, V, Owner, Value, ProjectedValue> Stream
    for &'a mut BusFieldPort<Owner, Value, ProjectedValue>
where
    V: CallAsync + ToOwned<Owned = Owner> + FieldRefController + NewFn<Inner = Owner> + 'a,
    Owner: Plugin<Virtual = V> + 'static,
    Value: 'static,
    ProjectedValue: 'static,
{
    type Item = (FieldRef<Owner, Value, ProjectedValue>, Owner);

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let filter = self.filter;
        let sel = self.owner_port.sel;

        let mut rx: tokio::sync::watch::Receiver<PacketRoutes<Owner>> =
            self.owner_port.client.subscribe();

        match rx.has_changed() {
            Ok(true) => {
                let next: tokio::sync::watch::Ref<'_, PacketRoutes<Owner>> = rx.borrow_and_update();

                let virt = next.virtual_ref();
                let field = sel(&virt);

                if filter(&field) {
                    // If field ref is in the committed state, notify any raw listeners
                    if field.is_committed() {
                        virt.owner().send_if_modified(|_| true);
                    }

                    std::task::Poll::Ready(Some((field.clone(), virt.to_owned())))
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
    }
}

#[async_trait]
pub trait VirtualBusExt: AsRef<ThunkContext> + AsMut<ThunkContext> {
    async fn virtual_bus(&self, address: impl Into<Address> + Send) -> VirtualBus
    where
        Self: Sync,
    {
        let address = address.into();

        if let Some(eh) = self.as_ref().engine_handle().await {
            eprintln!("Finding host");
            if let Ok(r) = eh.hosted_resource(address.to_string()).await {
                eprintln!("creating new bus");
                VirtualBus {
                    node: r.context().clone(),
                }
            } else {
                VirtualBus {
                    node: self.as_ref().clone(),
                }
            }
        } else {
            VirtualBus {
                node: self.as_ref().clone(),
            }
        }
    }
}

impl VirtualBusExt for ThunkContext {}
