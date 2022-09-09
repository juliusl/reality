use crate::{wire::{Encoder, ControlDevice}, WorldDir};

/// Extension api's for saving data to the world dir
///
pub trait Save {
    /// Saves an encoder to the world directory,
    /// 
    fn save_encoder(&self, encoder: Encoder);
}

impl Save for WorldDir {
    fn save_encoder(&self, encoder: Encoder) {
        // Create control device 
        let control = ControlDevice::from(&encoder);

        for frame in encoder.iter_frames() {
        }
        


        todo!()
    }
}
