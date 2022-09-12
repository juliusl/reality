use crate::{wire::FrameBus, WorldDir};

/// Extension api's for saving data to the world dir
///
pub trait Save {
    /// Starts a new frame bus for saving frames,
    /// 
    fn save_frames(&self) -> FrameBus;
}

impl Save for WorldDir {
    fn save_frames(&self) -> FrameBus {
        FrameBus::new(self.clone())
    }
}