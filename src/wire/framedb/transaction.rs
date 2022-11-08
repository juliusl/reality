use specs::{shred::ResourceId, Component, DenseVecStorage};
use tokio::io::DuplexStream;

use crate::{wire::{ControlDevice, Protocol, WireObject}, Block};

/// Component that represents a database transaction,
///
#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct Transaction {
    /// Control server/client
    ///
    pub control: (DuplexStream, DuplexStream),
    /// Frames server/client
    ///
    pub frames: (DuplexStream, DuplexStream),
    /// Blob server/client
    ///
    pub blob: (DuplexStream, DuplexStream),
    /// Control device
    ///
    pub control_device: ControlDevice,
    /// Resource-Id of the encoder to use,
    ///
    pub resource_id: ResourceId,
    /// If none, transaction is ready to commit,
    ///
    pub commit: Option<()>,
}

impl Transaction {
    pub async fn control(&mut self) -> &mut DuplexStream {
        &mut self.control.0
    }

    pub fn new() -> Self {
        todo!()
    }
}
