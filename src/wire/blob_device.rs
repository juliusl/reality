use std::{io::Cursor, collections::HashMap};

mod memory;
pub use memory::MemoryBlobSource;

mod file;
pub use file::FileBlobSource;

/// A blob source can create and find blob devices
///
pub trait BlobSource {
    /// Locates a blob device with `address` and
    /// returns a readonly cursor if it exists.
    ///
    fn read(&self, address: impl AsRef<str>) -> Option<&BlobDevice>;

    /// Locates a blob device with `address` and
    /// returns a writable cursor if it exists.
    ///
    fn write(&mut self, address: impl AsRef<str>) -> Option<&mut BlobDevice>;

    /// Returns a new cursor that can later be located w/
    /// `address`.
    ///
    /// If a cursor cannot be returned, than this method should
    /// panic.
    ///
    fn new(&mut self, address: impl AsRef<str>) -> &mut BlobDevice;

    /// Returns a hash_map clone of the current source 
    /// 
    fn hash_map(&self) -> HashMap<String, BlobDevice>;
}

/// This struct is to contain an addressable blob device
/// 
#[derive(Clone)]
pub struct BlobDevice {
    address: String,
    cursor: Cursor<Vec<u8>>,
}

impl BlobDevice {
    /// Returns a new blob device located at `address`
    ///
    pub fn new(address: impl AsRef<str>, cursor: Cursor<Vec<u8>>) -> Self {
        Self {
            address: address.as_ref().to_string(),
            cursor,
        }
    }

    /// Returns a new blob device, cloning an existing cursor
    ///
    pub fn existing(address: impl AsRef<str>, cursor: &Cursor<Vec<u8>>) -> Self {
        let mut clone = cursor.clone();
        clone.set_position(0);
        Self {
            address: address.as_ref().to_string(),
            cursor: clone,
        }
    }

    /// Returns the address used to locate this device
    ///
    pub fn address(&self) -> &String {
        &self.address
    }

    /// Returns an immutable reference to inner cursor 
    /// 
    pub fn cursor(&self) -> &Cursor<Vec<u8>> {
        &self.cursor
    }

    /// Consumes this device and returns the cursor
    ///
    pub fn consume(self) -> Cursor<Vec<u8>> {
        self.into()
    }
}

impl AsMut<Cursor<Vec<u8>>> for BlobDevice {
    fn as_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        &mut self.cursor
    }
}

impl AsRef<Cursor<Vec<u8>>> for BlobDevice {
    fn as_ref(&self) -> &Cursor<Vec<u8>> {
        &self.cursor
    }
}


impl Into<Cursor<Vec<u8>>> for BlobDevice {
    fn into(self) -> Cursor<Vec<u8>> {
        self.cursor
    }
}
