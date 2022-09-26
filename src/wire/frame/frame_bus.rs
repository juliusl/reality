use std::{path::Path, collections::HashMap};

use crate::{WorldDir, wire::{BlobSource, BlobDevice}};

use super::FrameDevice;

/// Struct to enumerate frame devices in priority order 
/// 
#[derive(Default)]
pub struct FrameBus {
    /// Devices in priority order
    /// 
    devices: Vec<FrameDevice>,
    /// Path to world directory 
    /// 
    world_dir: WorldDir, 
}

impl BlobSource for FrameBus {
    fn read(&self, address: impl AsRef<str>) -> Option<&BlobDevice> {
        todo!()
    }

    fn write(&mut self, address: impl AsRef<str>) -> Option<&mut BlobDevice> {
        todo!()
    }

    fn new(&mut self, address: impl AsRef<str>) -> &mut BlobDevice {
        todo!()
    }

    fn hash_map(&self) -> HashMap<String, BlobDevice> {
        let mut map = HashMap::default();
        for dev in self.devices.iter() {
            map.insert(dev.name().to_string(), dev.blob_device());
        }
        map
    }
}

impl FrameBus {
    /// Returns a new frame bus using a specific WorldDir
    /// 
    pub fn new(world_dir: WorldDir) -> Self {
        Self {
            devices: vec![],
            world_dir
        }
    }

    /// Commits all devices and returns the path to the stored frames
    /// 
    pub fn commit(&self) -> impl AsRef<Path>{
        /*
        Enumerate frame devices and write to disk
        */
        for (_idx, _dev) in self.devices.iter().enumerate() {

        }
        self.world_dir.dir()
    }
}
