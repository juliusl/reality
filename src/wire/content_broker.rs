use super::{BlobSource, MemoryBlobSource};

mod digest;
pub use digest::Sha256Digester;

/// A content broker formats blob devices from a blob source 
/// 
pub trait ContentBroker<Output = MemoryBlobSource> { 
    /// Formats a blob source
    /// 
    fn format(&mut self, source: impl BlobSource) -> Output;
}

