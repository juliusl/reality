use std::path::Path;

use crate::WorldDir;

use super::FrameDevice;

/// Struct to enumerate frame devices in priority order 
/// 
#[derive(Default, Clone)]
pub struct FrameBus {
    /// Devices in priority order 
    /// 
    devices: Vec<FrameDevice>,
    /// Path to world directory 
    /// 
    world_dir: WorldDir, 
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
