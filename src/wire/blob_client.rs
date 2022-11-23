use std::ops::Range;
use tokio::io::AsyncRead;

mod file_client;
pub use file_client::FileClient;

/// Trait to enable different blob client implementations downstream,
///
pub trait BlobClient: Clone + Send + Sync + 'static {
    /// An example stream type could be DuplexStream,
    ///
    type Stream: AsyncRead + Unpin + Send + Sync + 'static;

    /// Returns a stream to read a range of bytes,
    ///
    fn stream_range(&self, range: Range<usize>) -> Self::Stream;
}


