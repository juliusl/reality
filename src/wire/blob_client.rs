use std::ops::Range;
use tokio::io::AsyncRead;

/// Trait to enable different blob client implementations downstream,
/// 
pub trait BlobClient {
    /// An example stream type could be DuplexStream,
    /// 
    type Stream: AsyncRead + Unpin;

    /// Returns a stream to read a range of bytes,
    /// 
    fn stream_range(&self, range: Range<usize>) -> Self::Stream;
}