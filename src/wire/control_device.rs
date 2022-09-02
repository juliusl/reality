use super::{Frame, Interner};
use std::io::{Cursor, Write};
use tracing::{event, Level};

mod control_buffer;
pub use control_buffer::ControlBuffer;

/// Control device arranges control frames,
///
#[derive(Default)]
pub struct ControlDevice {
    data: Vec<Frame>,
    reads: Vec<Frame>,
}

impl ControlDevice {
    /// Returns a new control device from an interner,
    ///
    pub fn new(interner: Interner) -> Self {
        let mut control_device = ControlDevice::default();
        let mut control_buffer = ControlBuffer::default();
        for (_, ident) in interner.strings {
            control_buffer.add_string(ident);
        }

        let frames: Vec<Frame> = control_buffer.into();

        for frame in frames.iter() {
            if frame.op() == 0x00 {
                control_device.data.push(frame.clone());
            } else {
                control_device.reads.push(frame.clone());
            }
        }

        control_device
    }

    /// Returns the size in bytes of the control device,
    ///
    pub fn size(&self) -> usize {
        (self.data.len() + self.reads.len()) * 64
    }

    /// Returns an iterator over data frames,
    ///
    pub fn data_frames(&self) -> impl Iterator<Item = &Frame> {
        self.data.iter()
    }

    /// Returns an iterator over read frames,
    ///
    pub fn read_frames(&self) -> impl Iterator<Item = &Frame> {
        self.reads.iter()
    }

    /// Returns all of the data from data frames,
    ///
    pub fn data(&self) -> Cursor<Vec<u8>> {
        let mut cursor = Cursor::<Vec<u8>>::new(vec![]);

        for data in self.data_frames() {
            match cursor.write(&data.data()) {
                Ok(_) => {}
                Err(err) => event!(Level::ERROR, "could not write to cursor, {err}"),
            }
        }

        cursor.set_position(0);
        cursor
    }
}
