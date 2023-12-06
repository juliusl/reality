use std::marker::PhantomData;
use std::ops::Index;
use std::ops::IndexMut;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::OnceLock;

use anyhow::anyhow;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::error;
use tracing::trace;

use crate::Attribute;
use crate::AttributeType;
use crate::Dispatcher;
use crate::FieldRef;
use crate::FieldRefController;
use crate::NewFn;
use crate::OnParseField;
use crate::Plugin;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;

/// Field access,
///
#[derive(Debug)]
pub struct Field<'a, T> {
    /// Field owner type name,
    ///
    pub owner: &'static str,
    /// Name of the field,
    ///
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    ///
    pub value: &'a T,
}

/// Mutable field access,
///
#[derive(Debug)]
pub struct FieldMut<'a, T> {
    /// Field owner type name,
    ///
    pub owner: &'static str,
    /// Name of the field,
    ///
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Mutable access to the field,
    ///
    pub value: &'a mut T,
}

/// Field /w owned value,
///
#[derive(Debug)]
pub struct FieldOwned<T> {
    /// Field owner type name,
    ///
    pub owner: String,
    /// Name of the field,
    ///
    pub name: String,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    ///
    pub value: T,
}

/// Type-alias for a the frame version of an attribute type,
///
#[derive(Clone, Default, Debug)]
pub struct Frame {
    /// TODO: If set, acts as the receiver for any field packets,
    ///
    pub recv: FieldPacket,
    /// Packets for all fields,
    ///
    pub fields: Vec<FieldPacket>,
}

/// Wrapper struct over frames meant to update a block object,
///
/// TODO: Could add filters and such
///
#[derive(Clone, Debug, Default)]
pub struct FrameUpdates {
    pub frame: Frame,
}

/// Contains sync primitives for handling changes via framing,
///
/// A frame is a collection of field packets. Field packets can be applied to Plugin types to mutate field values.
///
/// # Protocol - Applying Packets
///
/// To apply a field packet, first the packet must be decoded by the virtual plugin into a field reference. The change
/// is simulated w/ the virtual plugin and if successful a field reference is returned.
///
/// This field reference represents a field that has a pending change. How the field reference is handled next is up
/// to the owner of the field reference. If the field reference is committed, it **MUST** mean that the value has been validated,
/// and that the value will be in use by the owner.
///
/// # Protocol - Sending packets
///
/// To broadcast a packet to a listener, first an update w/ the virtual plugin is applied. This results in a field
/// reference w/ the updated value. The field reference can then be encoded to a field packet and transmitted.
///
pub struct FrameListener<P: Plugin, const BUFFER_LEN: usize = 1>
where
    P::Virtual: NewFn<Inner = P>,
{
    /// Virtual reference,
    ///
    virt: Arc<tokio::sync::watch::Sender<PacketRoutes<P>>>,
    /// Packet receiver,
    ///
    rx_packets: Arc<Mutex<tokio::sync::mpsc::Receiver<Vec<FieldPacket>>>>,
    /// Packet transmission handle,
    ///
    tx_packets: Arc<tokio::sync::mpsc::Sender<Vec<FieldPacket>>>,
}

impl<P, const BUFFER_LEN: usize> Default for FrameListener<P, BUFFER_LEN>
where
    P: Plugin,
    P::Virtual: NewFn<Inner = P>,
{
    fn default() -> Self {
        FrameListener::new(P::default())
    }
}

impl<P, const BUFFER_LEN: usize> Clone for FrameListener<P, BUFFER_LEN>
where
    P: Plugin,
    P::Virtual: NewFn<Inner = P>,
{
    fn clone(&self) -> Self {
        Self {
            virt: self.virt.clone(),
            rx_packets: self.rx_packets.clone(),
            tx_packets: self.tx_packets.clone(),
        }
    }
}

impl<P: Plugin> FrameListener<P>
where
    P::Virtual: NewFn<Inner = P>,
{
    /// Returns a new frame listener w/ a specific buffer size,
    ///
    pub fn with_buffer<const BUFFER_LEN: usize>(init: P) -> FrameListener<P, BUFFER_LEN> {
        FrameListener::<P, BUFFER_LEN>::new(init)
    }
}

impl<P: Plugin, const BUFFER_LEN: usize> FrameListener<P, BUFFER_LEN>
where
    P::Virtual: NewFn<Inner = P>,
{
    /// Returns a new frame listener bounded by producer size,
    ///
    pub fn new(init: P) -> FrameListener<P, BUFFER_LEN> {
        let (wtx, _) = tokio::sync::watch::channel(PacketRoutes::new(init));
        let (tx, rx) = tokio::sync::mpsc::channel(BUFFER_LEN);

        FrameListener {
            virt: Arc::new(wtx),
            rx_packets: Arc::new(Mutex::new(rx)),
            tx_packets: Arc::new(tx),
        }
    }

    /// Returns a new listener w/ an updated buffer len,
    ///
    /// Re-uses the previous inner virtual plugin watch channel.
    ///
    /// **Panics** If NEW_BUFFER_LEN is < 0
    ///
    pub fn with_buffer_size<const NEW_BUFFER_LEN: usize>(self) -> FrameListener<P, NEW_BUFFER_LEN> {
        trace!(
            from = BUFFER_LEN,
            to = NEW_BUFFER_LEN,
            "Creating new listener w/ new buffer len"
        );

        let (tx, rx) = tokio::sync::mpsc::channel(NEW_BUFFER_LEN);

        FrameListener {
            virt: self.virt,
            rx_packets: Arc::new(Mutex::new(rx)),
            tx_packets: Arc::new(tx),
        }
    }

    /// Returns the max allowed producers,
    ///
    pub const fn buffer_len(&self) -> usize {
        BUFFER_LEN
    }

    /// Returns a channel to send field packets,
    ///
    pub fn frame_tx(&self) -> Arc<tokio::sync::mpsc::Sender<Vec<FieldPacket>>> {
        self.tx_packets.clone()
    }

    /// Returns a permit for transmitting a field packet when,
    ///
    pub async fn new_tx(&self) -> anyhow::Result<tokio::sync::mpsc::Permit<'_, Vec<FieldPacket>>> {
        let permit = self.tx_packets.reserve().await?;
        Ok(permit)
    }

    /// Listens for the next batch of packets,
    ///
    pub async fn listen(&mut self) -> anyhow::Result<Vec<FieldPacket>> {
        let mut rx = self.rx_packets.lock().await;

        match rx.recv().await {
            Some(packet) => Ok(packet),
            None => Err(anyhow!("Channel is closed")),
        }
    }

    /// Subscribes to active packet routes,
    ///
    pub fn subscribe_virtual(&self) -> tokio::sync::watch::Receiver<PacketRoutes<P>> {
        self.virt.subscribe()
    }

    /// Returns the current packet routes for this plugin,
    ///
    pub fn routes(&self) -> Arc<tokio::sync::watch::Sender<PacketRoutes<P>>> {
        self.virt.clone()
    }
}

/// Converts a type to a list of packets,
///
pub trait ToFrame: AttributeType<Shared> {
    /// Returns the current type as a Frame,
    ///
    fn to_frame(&self, key: ResourceKey<Attribute>) -> Frame;

    /// Returns the current type as a Frame w/ wire data set,
    ///
    fn to_wire_frame(&self, key: ResourceKey<Attribute>) -> Frame
    where
        Self: Sized + Serialize,
    {
        let mut frame = self.to_frame(key);
        frame.recv.wire_data = bincode::serialize(self).ok();
        frame
    }

    /// Returns an empty receiver packet,
    ///
    fn receiver_packet(&self, key: ResourceKey<Attribute>) -> FieldPacket
    where
        Self: Sized,
    {
        FieldPacket {
            data: None,
            wire_data: None,
            data_type_name: std::any::type_name::<Self>().to_string(),
            data_type_size: std::mem::size_of::<Self>(),
            field_offset: usize::MAX,
            field_name: Self::symbol().to_string(),
            owner_name: "self".to_string(),
            attribute_hash: Some(key.data),
            op: 0,
        }
    }
}

/// Struct for containing an object safe Field representation,
///
#[derive(Default, Serialize, Deserialize)]
pub struct FieldPacket {
    /// Pointer to data this packet has access to,
    ///
    #[serde(skip)]
    pub data: Option<Box<dyn FieldPacketType>>,
    /// Optional, wire data that can be used to create the field packet type,
    ///
    pub wire_data: Option<Vec<u8>>,
    /// Name of the type of data included
    ///
    pub data_type_name: String,
    /// Size of the type of data,
    ///
    pub data_type_size: usize,
    /// Field offset in the owning type,
    ///
    pub field_offset: usize,
    /// Name of the field,
    ///
    pub field_name: String,
    /// Type name of the owner of this field,
    ///
    pub owner_name: String,
    /// Attribute hash value,
    ///
    pub attribute_hash: Option<u128>,
    /// Operation code,
    /// (TODO)
    #[serde(skip)]
    op: u128,
}

impl Clone for FieldPacket {
    fn clone(&self) -> Self {
        Self {
            data: None,
            wire_data: self.wire_data.clone(),
            data_type_name: self.data_type_name.clone(),
            data_type_size: self.data_type_size,
            field_offset: self.field_offset,
            field_name: self.field_name.clone(),
            owner_name: self.owner_name.clone(),
            attribute_hash: self.attribute_hash,
            op: self.op,
        }
    }
}

impl std::fmt::Debug for FieldPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldPacket")
            .field("wire_data", &self.wire_data)
            .field("data_type_name", &self.data_type_name)
            .field("data_type_size", &self.data_type_size)
            .field("field_offset", &self.field_offset)
            .field("field_name", &self.field_name)
            .field("owner_name", &self.owner_name)
            .field("attribute_hash", &self.attribute_hash)
            .field("op", &self.op)
            .finish()
    }
}

impl FieldPacket {
    /// Creates a new packet w/o data,
    ///
    pub fn new<T>() -> Self {
        Self {
            wire_data: None,
            data: None,
            data_type_name: std::any::type_name::<T>().to_string(),
            data_type_size: std::mem::size_of::<T>(),
            field_name: String::new(),
            owner_name: String::new(),
            field_offset: 0,
            attribute_hash: None,
            op: 0,
        }
    }

    /// Creates a new packet w/ data to write a field with,
    ///
    pub fn new_data<T>(data: T) -> Self
    where
        T: FieldPacketType,
    {
        let mut packet = Self::new::<T>();
        if packet.data_type_name == std::any::type_name::<T>()
            && packet.data_type_size == std::mem::size_of::<T>()
        {
            packet.data = Some(Box::new(data));
            packet
        } else {
            packet
        }
    }

    /// Converts a field packet ptr into data,
    ///
    pub fn into_box<T>(self) -> Option<Box<T>>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        if self.data_type_name != std::any::type_name::<T>()
            || self.data_type_size != std::mem::size_of::<T>()
        {
            return None;
        }
        /// Convert a mut borrow to a raw mut pointer
        ///
        fn from_ref_mut<T: ?Sized>(r: &mut T) -> *mut T {
            r
        }

        if self.data.is_none() {
            if let Some(wire) = self.wire_data {
                return if let Ok(decoded) = bincode::deserialize(&wire) {
                    Some(Box::new(decoded))
                } else {
                    error!("Could not deserialize encoded value");
                    None
                };
            } else {
                error!("Field packet is completely empty");
                return None;
            }
        }

        self.data.and_then(|t| {
            let t = Box::leak(t);
            let t = from_ref_mut(t);
            let t = t.cast::<T>();
            if !t.is_null() {
                Some(unsafe { Box::from_raw(t) })
            } else {
                None
            }
        })
    }

    /// Converts packet into wire mode,
    ///
    pub fn into_wire<T>(self) -> FieldPacket
    where
        T: FieldPacketType + Sized + Serialize + DeserializeOwned,
    {
        let mut packet = FieldPacket {
            data: None,
            data_type_name: std::any::type_name::<T>().to_string(),
            data_type_size: std::mem::size_of::<T>(),
            field_offset: self.field_offset,
            field_name: self.field_name.to_string(),
            attribute_hash: self.attribute_hash,
            wire_data: None,
            owner_name: self.owner_name.to_string(),
            op: 0,
        };

        packet.wire_data = self.into_box::<T>().and_then(|d| d.to_binary().ok());
        packet
    }

    /// Sets the routing information for this packet,
    ///
    pub fn route(mut self, field_offset: usize, attribute: Option<u128>) -> Self {
        self.field_offset = field_offset;
        self.attribute_hash = attribute;
        self
    }

    /// Convert the packet into an owned field,
    ///
    pub fn into_field_owned(self) -> FieldOwned<FieldPacket> {
        FieldOwned {
            owner: self.owner_name.clone(),
            name: self.field_name.clone(),
            offset: self.field_offset,
            value: self,
        }
    }
}

/// Implemented by a type that can be stored into a packet,
///
pub trait FieldPacketType: Send + Sync + 'static {
    /// Type that can be serialized to/from a string,
    ///
    fn from_str_to_dest(str: &str, dest: &mut Option<Self>) -> anyhow::Result<()>
    where
        Self: FromStr + Sized;

    /// Type that can be deserialized to/from binary,
    ///
    fn from_binary(vec: Vec<u8>, dest: &mut Option<Self>) -> anyhow::Result<()>
    where
        Self: Serialize + DeserializeOwned,
    {
        let data = bincode::deserialize(&vec)?;
        let _ = dest.insert(data);
        Ok(())
    }

    /// Converts type to bincode bytes,
    ///
    fn to_binary(&self) -> anyhow::Result<Vec<u8>>
    where
        Self: Serialize + DeserializeOwned,
    {
        let ser = bincode::serialize(&self)?;
        Ok(ser)
    }
}

/// Trait for visiting fields w/ read-only access,
///
pub trait Visit<T> {
    /// Returns a vector of fields,
    ///
    fn visit(&self) -> Vec<Field<'_, T>>;
}

/// Trait for visiting fields w/ mutable access,
///
pub trait VisitMut<T> {
    /// Returns a vector of fields w/ mutable access,
    ///
    fn visit_mut<'a: 'b, 'b>(&'a mut self) -> Vec<FieldMut<'b, T>>;
}

/// Trait for visiting fields references on the virtual reference,
///
pub trait VisitVirtual<T, Projected>
where
    Self: Plugin + 'static,
    T: 'static,
    Projected: 'static,
{
    /// Returns a vector of field references from the virtual plugin,
    ///
    fn visit_fields<'a>(virt: &'a PacketRoutes<Self>) -> Vec<&'a FieldRef<Self, T, Projected>>;
}

/// Trait for visiting fields references on the virtual reference,
///
pub trait VisitVirtualMut<T, Projected>
where
    Self: Plugin + 'static,
    T: 'static,
    Projected: 'static,
{
    /// Returns a vector of mutable field references from the virtual plugin,
    ///
    fn visit_fields_mut(
        routes: &mut Self::Virtual,
        visit: impl FnMut(&mut FieldRef<Self, T, Projected>),
    );
}

/// Trait for setting a field,
///
pub trait SetField<T> {
    /// Sets a field on the receiver,
    ///
    /// Returns true if successful.
    ///
    fn set_field(&mut self, field: FieldOwned<T>) -> bool;
}

impl<T> FieldPacketType for T
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    fn from_str_to_dest(str: &str, dest: &mut Option<Self>) -> anyhow::Result<()>
    where
        Self: FromStr + Sized,
    {
        if let Ok(value) = <T as FromStr>::from_str(str) {
            let _ = dest.insert(value);
        }
        Ok(())
    }
}

/// Wrapper over the field offset and type so that the compiler can match by offset,
///
pub struct FieldIndex<const OFFSET: usize, T>(pub PhantomData<T>);

/// Wraps a field offset into a pointer struct,
///
/// Used to convert into a field index.
///
pub struct FieldKey<const OFFSET: usize>;

/// Wrapper over an inner virtual plugin providing an index operation for
/// finding packet routes.
///
pub struct PacketRoutes<P: Plugin> {
    /// Inner virtual plugin,
    ///
    pub inner: P::Virtual,
}

/// When a packet arrives to the router, it's decoded by each field to find the field it applies to.
/// 
/// If the field was updated successfully, it is passed to the dispatcher which will then update FrameUpdates.
/// 
/// FrameUpdates are read when a remote plugin is loaded and applied to the initialized state of the plugin.
/// 
pub struct PacketRouter<P, S = Shared>
where
    P: Plugin,
    S: StorageTarget + Send + Sync + 'static,
{
    /// Handle to a watch channel which maintains state of P,
    /// 
    /// The inner PacketRoutes is just a wrapper over the Virtual plugin that provides access to field refs
    /// by offset.
    ///
    pub routes: Arc<tokio::sync::watch::Sender<PacketRoutes<P>>>,
    /// Dispatcher,
    ///
    pub dispatcher: OnceLock<Dispatcher<S, FrameUpdates>>,
    /// Broadcast channel for forwarding packets to routes,
    /// 
    pub tx: Arc<tokio::sync::broadcast::Sender<FieldPacket>>,
}

impl<P, S> PacketRouter<P, S>
where
    P: Plugin,
    S: StorageTarget + Send + Sync + 'static,
{
    /// Creates a new packet router,
    ///
    pub fn new(routes: Arc<tokio::sync::watch::Sender<PacketRoutes<P>>>) -> Self
    where
        P::Virtual: NewFn<Inner = P>,
    {
        let len = P::default().to_frame(ResourceKey::new()).fields.len();

        let (tx, _rx) = tokio::sync::broadcast::channel(usize::max(len, 1));

        Self {
            routes,
            tx: Arc::new(tx),
            dispatcher: OnceLock::new(),
        }
    }

    /// Routes a single packet to OFFSET,
    ///
    /// Returns Ok(()) if the packet was received and dispatched successfully, otherwise
    /// returns an error.
    ///
    pub async fn route_one<const OFFSET: usize>(&self) -> anyhow::Result<()>
    where
        P: OnWriteField<OFFSET>
            + OnReadField<OFFSET>
            + OnParseField<OFFSET, <P as OnReadField<OFFSET>>::FieldType>
            + Plugin,
        P::Virtual: NewFn<Inner = P>,
    {
        if self.dispatcher.get().is_some() {
            let mut rx = self.tx.clone().subscribe();

            let next = rx.recv().await?;

            self.try_route(next).await?;

            Ok(())
        } else {
            Err(anyhow!("Not bound to a dispatcher"))
        }
    }

    /// Tries to route the packet to field at OFFSET,
    ///
    /// If the packet can be decoded and applied, and a change was applied, a dispatch
    /// is queued that pushes the update to frame updates.
    ///
    pub async fn try_route<const OFFSET: usize>(&self, packet: FieldPacket) -> anyhow::Result<()>
    where
        P: OnReadField<OFFSET>
            + OnWriteField<OFFSET>
            + OnParseField<OFFSET, <P as OnReadField<OFFSET>>::FieldType>
            + Plugin,
        P::Virtual: NewFn<Inner = P>,
    {
        if let Some(mut dispatcher) = self.dispatcher.get().cloned() {
            let routes = self.routes.borrow();

            let field_ref = routes.route::<OFFSET>();

            // It's possible this packet comes from outside of the process. Log what we are checking against
            trace!(
                field_offset_src = OFFSET,
                field_name_src = field_ref.encode().field_name,
                field_offset_remote = packet.field_offset,
                field_name_remote = packet.field_name,
                "Filtering packet",
            );
            match field_ref.filter_packet(&packet) {
                Ok(field) => {
                    // When this is queued to the dispatcher, the next time the remote_plugin is loaded
                    // the dispatcher will drain this queue and frame updates will be updated
                    dispatcher.queue_dispatch_mut(move |f| {
                        f.frame.fields.push(field.encode());
                    });
                    return Ok(());
                }
                Err(err) => {
                    trace!(
                        "Skipping packet, {err}",
                    );
                },
            }
            Err(anyhow!("Did not apply packet via this route"))
        } else {
            Err(anyhow!("Not bound to a dispatcher"))
        }
    }
}

impl<P: Plugin> PacketRoutes<P> {
    /// Creates a new packet routes interface,
    ///
    pub fn new(inner: P) -> Self
    where
        P::Virtual: NewFn<Inner = P>,
    {
        PacketRoutes {
            inner: P::Virtual::new(inner),
        }
    }

    pub fn apply_pending_list(&mut self, list: &[&str]) {
        for l in list {
            self.inner.set_pending(&l);
        }
    }

    /// Returns a reference to a field reference for the field at OFFSET,
    ///
    pub fn route<const OFFSET: usize>(
        &self,
    ) -> &FieldRef<
        P,
        <P as OnReadField<OFFSET>>::FieldType,
        <P as OnParseField<OFFSET, <P as OnReadField<OFFSET>>::FieldType>>::ProjectedType,
    >
    where
        P: OnReadField<OFFSET>
            + OnWriteField<OFFSET>
            + OnParseField<OFFSET, <P as OnReadField<OFFSET>>::FieldType>
            + Plugin,
    {
        &self[FieldKey::<OFFSET>.into()]
    }

    pub fn route_mut<'a: 'b, 'b, const OFFSET: usize>(
        &'a mut self,
    ) -> &'b mut FieldRef<
        P,
        <P as OnReadField<OFFSET>>::FieldType,
        <P as OnParseField<OFFSET, <P as OnReadField<OFFSET>>::FieldType>>::ProjectedType,
    >
    where
        P: OnReadField<OFFSET>
            + OnWriteField<OFFSET>
            + OnParseField<OFFSET, <P as OnReadField<OFFSET>>::FieldType>
            + Plugin,
    {
        &mut self[FieldKey::<OFFSET>.into()]
    }
}

impl<const OFFSET: usize, P, T> IndexMut<FieldIndex<OFFSET, T>> for PacketRoutes<P>
where
    T: Send + Sync + 'static,
    P: OnReadField<OFFSET, FieldType = T>
        + OnWriteField<OFFSET, FieldType = T>
        + OnParseField<OFFSET, T>
        + Plugin,
{
    fn index_mut(&mut self, _: FieldIndex<OFFSET, T>) -> &mut Self::Output {
        P::write(&mut self.inner)
    }
}

impl<const OFFSET: usize, P, T> Index<FieldIndex<OFFSET, T>> for PacketRoutes<P>
where
    T: Send + Sync + 'static,
    P: OnReadField<OFFSET, FieldType = T>
        + OnWriteField<OFFSET, FieldType = T>
        + OnParseField<OFFSET, T>
        + Plugin,
{
    type Output = FieldRef<P, T, <P as OnParseField<OFFSET, T>>::ProjectedType>;

    #[inline]
    fn index(&self, _: FieldIndex<OFFSET, T>) -> &Self::Output {
        P::read(&self.inner)
    }
}

impl<const OFFSET: usize, T> From<FieldKey<OFFSET>> for FieldIndex<OFFSET, T> {
    fn from(_: FieldKey<OFFSET>) -> Self {
        FieldIndex(PhantomData)
    }
}

/// Trait for returning field references by offset,
///
pub trait OnReadField<const OFFSET: usize>
where
    Self: Plugin + OnParseField<OFFSET, <Self as OnReadField<OFFSET>>::FieldType>,
{
    /// The field type being read,
    ///
    type FieldType: Send + Sync + 'static;

    /// Reads a field reference from this type,
    ///
    fn read(virt: &Self::Virtual) -> &FieldRef<Self, Self::FieldType, Self::ProjectedType>;
}

/// Trait for returning mutable field references by offset,
///
pub trait OnWriteField<const OFFSET: usize>
where
    Self: Plugin
        + OnReadField<OFFSET>
        + OnParseField<OFFSET, <Self as OnReadField<OFFSET>>::FieldType>,
{
    /// Writes to a field reference from this type,
    ///
    fn write(virt: &mut Self::Virtual)
        -> &mut FieldRef<Self, Self::FieldType, Self::ProjectedType>;
}

#[allow(unused_imports)]
mod tests {
    use std::{
        ops::Index,
        sync::{Arc, OnceLock},
        time::Duration,
    };

    use super::FieldMut;
    use crate::{prelude::*, FieldKey, FrameListener, PacketRoutes};

    pub mod reality {
        pub use crate::*;
        pub mod prelude {
            pub use crate::prelude::*;
        }
    }

    use anyhow::anyhow;
    use async_stream::stream;
    use async_trait::async_trait;
    use futures_util::{pin_mut, StreamExt};
    use serde::Serialize;
    use tokio::{join, time::Instant};

    #[derive(Reality, Clone, Serialize, Default)]
    #[reality(call=test_noop, plugin)]
    struct Test {
        #[reality(derive_fromstr)]
        name: String,
        other: String,
    }

    async fn test_noop(_tc: &mut ThunkContext) -> anyhow::Result<()> {
        Ok(())
    }

    #[test]
    fn test_visit() {
        let mut test = Test {
            name: String::from(""),
            other: String::new(),
        };
        {
            let mut fields = test.visit_mut();
            let mut fields = fields.drain(..);
            if let Some(FieldMut { name, value, .. }) = fields.next() {
                assert_eq!("name", name);
                *value = String::from("hello-world");
            }

            if let Some(FieldMut { name, value, .. }) = fields.next() {
                assert_eq!("other", name);
                *value = String::from("hello-world-2");
            }
        }

        assert_eq!("hello-world", test.name.as_str());
        assert_eq!("hello-world-2", test.other.as_str());
    }

    #[test]
    fn test_packet() {
        let packet = crate::attributes::visit::FieldPacket::new_data(String::from("Hello World"));
        let packet = packet.into_box::<String>();
        let packet_data = packet.expect("should be able to convert");
        let packet_data = packet_data.as_str();
        assert_eq!("Hello World", packet_data);

        let packet = crate::attributes::visit::FieldPacket::new_data(String::from("Hello World"));
        let packet = packet.into_box::<Vec<u8>>();
        assert!(packet.is_none());

        let packet = crate::attributes::visit::FieldPacket::new_data(String::from("Hello World"));
        let packet = packet.route(0, None).into_wire::<String>();
        println!("{:?}", packet.wire_data);
    }

    #[tokio::test]
    async fn test_frame_listener() {
        let mut _frame_listener = FrameListener::with_buffer::<1>(Test {
            name: String::from("cool name"),
            other: String::from("hello other world"),
        });

        let tx = _frame_listener.routes();

        let field_ref = tx
            .borrow()
            .inner
            .name
            .clone()
            .start_tx()
            .next(|f| {
                assert!(f.edit_value(|_, v| {
                    *v = String::from("really cool name");
                    true
                }));

                Ok(f)
            })
            .finish()
            .unwrap();

        let packet = field_ref.encode();

        let permit = _frame_listener.new_tx().await.unwrap();
        permit.send(vec![packet]);

        let next = _frame_listener.listen().await.unwrap();
        eprintln!("{:#?}", next);
        ()
    }

    #[tokio::test]
    async fn test_frame_router() {
        // Create a new node
        let node = Shared::default().into_thread_safe_with(tokio::runtime::Handle::current());

        // Simulate a thunk context being used
        let mut tc: ThunkContext = node.into();

        // Create a new wire server/client for this plugin Test
        let server = WireServer::<Test>::new(&mut tc).await.unwrap();

        tokio::spawn(server.clone().start());

        let client = server.clone().new_client();

        client
            .try_borrow_modify(|t| {
                t.inner.name.edit_value(|_, n| {
                    *n = String::from("hello world cool test 2");
                    true
                });

                Ok(t.inner.name.encode())
            })
            .unwrap();

        // Simulate a concurrent process starting up subsequently
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate receiving changes on thunk execution
        let test = Remote.create::<Test>(&mut tc).await;

        test.to_virtual().name.view_value(|v| {
            assert_eq!("hello world cool test 2", v);
        });

        client
            .try_borrow_modify(|t| {
                t.inner.name.edit_value(|_, n| {
                    *n = String::from("hello world cool test 3");
                    true
                });
                Ok(t.inner.name.encode())
            })
            .unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate receiving changes on thunk execution
        let test = Remote.create::<Test>(&mut tc).await;

        test.to_virtual().name.view_value(|v| {
            assert_eq!("hello world cool test 3", v);
        });

        ()
    }
}
