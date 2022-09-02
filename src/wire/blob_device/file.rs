use std::{path::PathBuf, io::Cursor, fs::File};

use tracing::{event, Level};

use super::BlobSource;

/// Blob source using a file system, 
/// 
/// The point of a blob source is to fetch immutable content, 
/// since the content is immutable that should also mean it is content addressable. 
/// 
/// Filesystems typically use file paths to address content (at least in 
/// the first tier of file interfaces.) However, a file path is typically not enough
/// to convey a content address. The main reason of course is that 
/// filesystem implementations are mainly concerned with reliability, rather
/// than focusing on being able to fetch data via a content address. 
/// 
/// So then the goal of the file blob source is to try and bridge this gap. 
/// 
/// The challenge becomes, how to design an address around these requirements.
/// 
/// The solution is to rely on a versioning system. Versioning systems such as git
/// already must be able to solve this problem in order to track the version of a 
/// file. Not only that, the plumbing to be able to do this cross platform, consistently
/// must exist as well.
/// 
/// In this context, a good enough candidate is a command to a version control system. 
/// 
/// For example, if we wanted to index a file such as `./readme.md`, and we're using 
/// git, then the address could be something like `git hash-object ./readme.md | <hash-value>`
/// 
/// This creates a clean interface between the filesystem and the application that requires 
/// the dependency.  
/// 
pub struct FileBlobSource {
}

/// Address function
/// 
pub type Address = fn(Cursor<File>) -> Option<String>;

impl FileBlobSource {
    /// Opens a file to add to the blob source 
    /// 
    pub fn add_file(&mut self, filepath: impl AsRef<PathBuf>, address: Address) {
        match File::open(filepath.as_ref()) {
            Ok(file) => {
                if let Some(address) = address(Cursor::new(file)) {
                    /*
                    
                    */
                }
            },
            Err(err) => {
                event!(Level::ERROR, "could not open file, {err}");
            },
        }
    }
}

impl BlobSource for FileBlobSource {
    /// 
    /// 
    fn read(&self, address: impl AsRef<str>) -> Option<&super::BlobDevice> {
        todo!()
    }

    /// 
    /// 
    fn write(&mut self, address: impl AsRef<str>) -> Option<&mut super::BlobDevice> {
        todo!()
    }

    /// 
    /// 
    fn new(&mut self, address: impl AsRef<str>) -> &mut super::BlobDevice {
        todo!()
    }

    /// 
    /// 
    fn hash_map(&self) -> std::collections::HashMap<String, super::BlobDevice> {
        todo!()
    }
}