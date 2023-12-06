use std::{ops::{IndexMut, Index}, marker::PhantomData, sync::{Arc, OnceLock}};

use tracing::trace;
use anyhow::anyhow;

use crate::{OnReadField, OnWriteField, OnParseField, Plugin, FieldRef, Shared, StorageTarget, Dispatcher, FrameUpdates, NewFn, ResourceKey, FieldRefController};

use super::prelude::FieldPacket;


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
    pub tx: Arc<tokio::sync::broadcast::Sender<super::packet::FieldPacket>>,
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
