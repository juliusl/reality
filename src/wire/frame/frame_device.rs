use std::io::{Cursor, Write};

use tracing::{event, Level};

use crate::wire::BlobDevice;

use super::Frame;

/// Struct to store a vector of frames w/ a name
///
#[derive(Clone, Debug)]
pub struct FrameDevice {
    /// Name of the device
    name: String,
    /// Frames this device is storing
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

    /// Returns a blob device inferred from frames in this device,
    /// 
    pub fn blob_device(&self) -> BlobDevice {
        let mut blob_device = BlobDevice::new(
            self.name.to_string(), 
            Cursor::new(vec![])
        );

        for f in self.iter_frames() {
            match blob_device.as_mut().write_all(f.bytes()) {
                Ok(_) => event!(Level::TRACE, "Wrote frame {f}"),
                Err(err) => event!(Level::ERROR, "Error writing to blob device {err}"),
            }
        }

        blob_device
    }

    /// Returns the name of the device
    ///
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Returns the number of frames 
    /// 
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Returns an iterator over frames in the device
    ///
    pub fn iter_frames(&self) -> impl Iterator<Item = &Frame> {
        self.frames.iter()
    }
}

impl From<BlobDevice> for FrameDevice {
    fn from(blob_device: BlobDevice) -> Self {
        let name = blob_device.address().to_string();
        let blob = blob_device.consume().into_inner();

        let mut frames = vec![];
        let mut frame_chunks = blob.chunks_exact(64);
        while let Some(chunk) = frame_chunks.next() {
            let frame = Frame::from(chunk);
            frames.push(frame);
        }

        // This device must be exact
        assert!(frame_chunks.remainder().is_empty());
        
        FrameDevice::new(name, frames.iter())
    }
}
