use super::Frame;

/// Struct to store a vector of frames w/ a name
///
#[derive(Clone, Debug)]
pub struct FrameDevice {
    name: String,
    frames: Vec<Frame>,
}

impl FrameDevice {
    /// Creates a new frame device
    ///
    pub fn new<'a>(name: impl AsRef<str>, frames: impl Iterator<Item = &'a Frame>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            frames: frames.cloned().collect(),
        }
    }

    /// Returns the name of the device
    ///
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Returns an iterator over frames in the device
    ///
    pub fn iter_frames(&self) -> impl Iterator<Item = &Frame> {
        self.frames.iter()
    }
}
