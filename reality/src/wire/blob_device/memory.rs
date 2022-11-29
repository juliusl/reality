use std::{
    collections::HashMap,
    io::Cursor,
};

use tracing::{event, Level};

use super::{BlobDevice, BlobSource};

/// Blob source in memory,
/// 
/// This blob source can copy devices from other sources,
///
#[derive(Default, Clone)]
pub struct MemoryBlobSource {
    devices: HashMap<String, BlobDevice>,
}

impl MemoryBlobSource {
    /// Copies an existing blob device
    ///
    pub fn copy_device(&mut self, device: &BlobDevice) {
        match self.devices.insert(
            device.address().to_string(),
            BlobDevice::existing(device.address(), device.as_ref()),
        ) {
            Some(existing) => {
                event!(
                    Level::INFO,
                    "overwriting existing device at {}",
                    existing.address()
                );
            }
            None => {
                event!(Level::DEBUG, "copied device for {}", device.address());
            }
        }
    }

    /// Copies blob from source, if select returns true
    /// 
    pub fn copy_select(
        &mut self, 
        source: impl BlobSource, 
        select: impl Fn(&String, &BlobDevice) -> bool) {
            for (address, device) in source.hash_map().iter().filter(|(a, d)| {
                select(a, d)
            }) {
                event!(Level::DEBUG, "copying device {address}");
                self.copy_device(device);
            }
    }

    /// Copies all blob devices from source
    /// 
    pub fn copy_source(&mut self, source: impl BlobSource) {
        self.copy_select(source, |_, _| true)
    }
}

impl BlobSource for MemoryBlobSource {
    fn read(&self, address: impl AsRef<str>) -> Option<&super::BlobDevice> {
        // Note: The cursor is not reset, the cursor is returned as is
        //
        self.devices.get(address.as_ref())
    }

    fn write(&mut self, address: impl AsRef<str>) -> Option<&mut super::BlobDevice> {
        self.devices.get_mut(address.as_ref())
    }

    fn new(&mut self, address: impl AsRef<str>) -> &mut super::BlobDevice {
        self.devices.insert(
            address.as_ref().to_string(),
            BlobDevice::new(&address, Cursor::new(vec![])),
        );

        self.devices.get_mut(address.as_ref()).expect("just added")
    }

    fn hash_map(&self) -> HashMap<String, BlobDevice> {
        self.devices.clone()
    }
}

/// Tests the basic functionality of the blob source trait
/// and a blob device struct
///
#[test]
fn test_memory_blob_source() {
    let mut blob_source = MemoryBlobSource::default();

    // Tests creating a new blob device
    {
        let test_blob = blob_source.new("test_blob");
        let written = std::io::Write::write(&mut test_blob.as_mut(), b"hello world").expect("writable");
        assert_eq!(written, 11);
    }

    // Tests reading from an existing blob device
    {
        let test_blob = blob_source.read("test_blob").expect("can read");
        let test_blob = test_blob.as_ref().clone().into_inner();
        assert_eq!(&test_blob, b"hello world");
    }

    // Tests writing to an existing blob device
    {
        let test_blob = blob_source.write("test_blob").expect("can read");
        let written = std::io::Write::write(&mut test_blob.as_mut(), b"hello world").expect("writable");
        assert_eq!(written, 11);
    }

    // Tests reading from an existing blob device
    {
        let test_blob = blob_source.read("test_blob").expect("can read");
        let test_blob = test_blob.as_ref().clone().into_inner();
        assert_eq!(&test_blob, b"hello worldhello world");
    }

    // Tests copying a blob device 
    {
        let mut destination = MemoryBlobSource::default();

        let test_blob = blob_source.read("test_blob").expect("can read");
        destination.copy_device(test_blob);

        let test_blob = destination.read("test_blob").expect("can read");
        let test_blob = test_blob.clone().consume().into_inner();
        assert_eq!(&test_blob, b"hello worldhello world");
    }

    // Tests copying an entire blob source 
    {
        let mut destination = MemoryBlobSource::default();
        destination.copy_source(blob_source);

        let test_blob = destination.read("test_blob").expect("can read");
        let test_blob = test_blob.clone().consume().into_inner();
        assert_eq!(&test_blob, b"hello worldhello world");
    }
}
