use std::ops::Range;
use tokio::io::AsyncRead;

/// Trait to enable different blob client implementations downstream,
/// 
pub trait BlobClient {
    /// Returns a stream to read a range of bytes,
    /// 
    fn stream_range<S>(&self, range: Range<usize>) -> S
    where
        S: AsyncRead + Unpin;
}