use anyhow::anyhow;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::trace;

use crate::Attribute;
use crate::AttributeType;
use crate::FieldPacket;
use crate::LocalAnnotations;
use crate::NewFn;
use crate::PacketRoutes;
use crate::Plugin;
use crate::ResourceKey;
use crate::Shared;

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
    /// Frame to apply,
    ///
    pub frame: Frame,
    /// Local annotations to apply w/ frame updates,
    ///
    pub annotations: LocalAnnotations,
}

impl FrameUpdates {
    /// Sets a property on local annotations,
    ///
    pub fn set_property(&mut self, k: impl Into<String>, v: impl Into<String>) {
        self.annotations.map.insert(k.into(), v.into());
    }

    /// Returns true if an update exists,
    ///
    pub fn has_update(&self) -> bool {
        self.frame.fields.is_empty() || self.annotations.map.is_empty()
    }
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
