use std::{path::PathBuf, sync::Arc, ops::Range, io::SeekFrom};

use tokio::io::{DuplexStream, AsyncSeekExt, AsyncReadExt, AsyncWriteExt};
use tracing::{event, Level};

use super::BlobClient;

/// Struct that implements BlobClient with a file,
///
#[derive(Clone)]
pub struct FileClient {
    path: Arc<PathBuf>,
}

impl BlobClient for FileClient {
    type Stream = DuplexStream;

    fn stream_range(&self, range: Range<usize>) -> Self::Stream {
        let (mut tx, rx) = tokio::io::duplex(range.len());

        let path = self.path.clone();
        tokio::spawn(async move {
            match tokio::fs::File::open(path.as_ref()).await {
                Ok(mut file) => {
                    file.seek(SeekFrom::Start(range.start as u64))
                        .await
                        .unwrap_or(0);

                    let file_size = file.metadata().await.expect("should have metadata").len();
                    assert!((file_size - range.start as u64) <= range.len() as u64, "Range exceeds remaining file size");

                    let mut buf = vec![0; range.len()];
                    let read = file.read_exact(&mut buf).await.unwrap_or(0);
                    debug_assert_eq!(read, range.len(), "Could not read from file");

                    file.flush().await.expect("should be able to flush");
                    let read = tokio::io::copy(&mut buf.as_ref(), &mut tx)
                        .await
                        .unwrap_or(0);
                    debug_assert_eq!(read, range.len() as u64, "Could not copy bytes");
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not open file, {err}")
                },
            }
        });

        rx
    }
}

#[tokio::test]
async fn test_file_blob_client() {
    let client = FileClient {
        path: Arc::new(PathBuf::from("Cargo.toml")),
    };

    let mut bytes = vec![];
    assert_eq!(
        client
            .stream_range(0..50)
            .read_to_end(&mut bytes)
            .await
            .expect("can read"),
        (0..50).len()
    );
}