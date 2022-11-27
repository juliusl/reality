use std::{sync::Arc, ops::Range};

use bytes::{Bytes, BytesMut};
use futures::Future;
use tokio::io::{DuplexStream, AsyncReadExt, AsyncWriteExt};
use tokio_stream::StreamExt;
use tracing::{event, Level};

use crate::wire::{BlockClient, Interner, Encoder, FrameBuffer, Frame, BlockEntry};

use super::{StoreIndex, StoreKey};

/// Struct for an entry in the index,
///
pub struct Entry<Client>
where
    Client: BlockClient,
{
    /// Parent index of this entry,
    ///
    index: Arc<StoreIndex<Client>>,
    /// Store key of this entry,
    ///
    store_key: StoreKey,
    /// Interner,
    ///
    interner: Arc<Interner>,
    /// Start byte range,
    ///
    start: usize,
    /// End byte range,
    ///
    end: usize,
    /// Cached bytes for this entry,
    ///
    cache: Option<Bytes>,
}

impl<Client> Entry<Client>
where
    Client: BlockClient,
{
    /// Returns a new index entry,
    /// 
    pub fn new(index: Arc<StoreIndex<Client>>, interner: Arc<Interner>, key: &StoreKey, range: Range<usize>) -> Self {
        Self {
            index,
            interner,
            store_key: key.clone(),
            start: range.start,
            end: range.end,
            cache: None,
        }
    }
    /// Returns the start of this entry,
    /// 
    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the end of this entry,
    /// 
    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Returns the name of the entry,
    ///
    #[inline]
    pub fn key(&self) -> &StoreKey {
        &self.store_key
    }

    /// Returns the stored size of this entry,
    ///
    #[inline]
    pub fn size(&self) -> usize {
        self.end - self.start
    }

    /// Returns the name of this entry,
    ///
    #[inline]
    pub fn name(&self) -> Option<&String> {
        self.key().name(&self.interner)
    }

    /// Returns the symbol of this entry,
    ///
    #[inline]
    pub fn symbol(&self) -> Option<&String> {
        self.key().symbol(&self.interner)
    }

    /// Returns a reader to pull bytes for this entry,
    ///
    pub fn pull(&self) -> Client::Stream {
        self.index.client().stream_range(self.start..self.end)
    }

    /// Returns true if this entry has a blob device,
    ///
    pub fn has_blob_device(&self) -> bool {
        if let Some(keys) = self
            .symbol()
            .and_then(|s| self.index.blob_devices(s))
        {
            !keys.is_empty()
        } else {
            false
        }
    }

    /// Returns a fully loaded encoder for this entry,
    ///
    pub async fn encoder(&self) -> Option<Encoder> {
        if let Some(mut stream) = self.stream_blob_device(4096) {
            let mut encoder = Encoder::new();

            match stream.read_to_end(encoder.blob_device.get_mut()).await {
                Ok(read) => {
                    event!(Level::TRACE, "Read {read} bytes");
                }
                Err(err) => {
                    event!(Level::ERROR, "Error reading blob device stream, {err}");
                }
            }

            encoder.interner = self.index.interner().clone();

            let mut frames = self.pull();

            let mut frame_buffer = FrameBuffer::new(100);

            let mut b = frame_buffer.next();
            while let Ok(read) = frames.read_exact(b.as_mut()).await {
                assert_eq!(read, 64);
                encoder.frames.push(Frame::from(b.freeze()));
                b = frame_buffer.next();
            }

            Some(encoder)
        } else {
            None
        }
    }

    /// Return blob device entries that belong to this entry,
    ///
    pub fn iter_blob_entries(&self) -> impl Iterator<Item = Entry<Client>> + '_ {
        let keys = if self.has_blob_device() {
            let key = self.symbol().expect("should have a symbol");
            self.index
                .blob_devices(key)
                .expect("should have keys")
                .clone()
        } else {
            vec![]
        };

        keys.into_iter().filter_map(|k| self.index.entry(k))
    }

    /// If entry has child blob entries, concatenates each into a single stream,
    ///
    pub fn stream_blob_device(&self, buffer_size: usize) -> Option<DuplexStream> {
        if self.has_blob_device() {
            let (mut sender, receiver) = tokio::io::duplex(buffer_size);

            let mut blocks = futures::stream::FuturesOrdered::new();

            for entry in self.iter_blob_entries() {
                blocks.push_back(async move { entry.bytes().await });
            }

            tokio::task::spawn(async move {
                while let Some(next) = blocks.next().await {
                    let next_len = next.len();

                    match sender.write_all(next.as_ref()).await {
                        Ok(_) => {
                            event!(Level::TRACE, "Sent {next_len} bytes");
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Error sending bytes, {err}");
                        }
                    }
                }
            });

            Some(receiver)
        } else {
            None
        }
    }

    /// Manually join's blob devices with an accumalator `T`,
    ///
    /// Call on_blob on each blob read from blob device entries. Blobs are processed in order,
    /// but are fetched in parallel.
    ///
    pub async fn join_blob_device<T, F>(
        &self,
        mut acc: T,
        on_blob: impl Fn(T, Entry<Client>, Bytes) -> F,
    ) -> T
    where
        F: Future<Output = T>,
    {
        if self.has_blob_device() {
            let mut blocks = futures::stream::FuturesOrdered::new();

            for entry in self.iter_blob_entries() {
                blocks.push_back(async move {
                    let bytes = entry.bytes().await;
                    (entry, bytes)
                });
            }

            while let Some((entry, next)) = blocks.next().await {
                acc = on_blob(acc, entry, next).await;
            }
        }

        acc
    }

    /// Caches bytes for this entry,
    ///
    /// This can be useful if this entry isn't being unpacked to the filesystem, but this entry
    /// will be around long enough to be called multiple times for bytes.
    ///  
    pub async fn cache(&mut self) {
        let mut size = self.end - self.start;
        let mut bytes = BytesMut::with_capacity(self.end - self.start);

        while size > 0 {
            match self.pull().read_buf(&mut bytes).await {
                Ok(read) => {
                    event!(Level::TRACE, "Read {read} bytes");
                    size -= read;
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not cache entry, {err}");
                }
            }
        }

        self.cache = Some(bytes.into());
    }

    /// Returns bytes for this entry,
    ///
    /// If a cache exists, returns the cached version
    ///
    pub async fn bytes(&self) -> Bytes {
        if let Some(cached) = self.cache.as_ref() {
            cached.clone()
        } else {
            let mut bytes = self.pull();

            let mut buf = vec![];

            match bytes.read_to_end(&mut buf).await {
                Ok(read) => {
                    event!(Level::TRACE, "Read {read} bytes");
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not read blob, {err}");
                }
            }

            buf.into()
        }
    }

    /// Returns a hash code for this entry's key,
    ///
    pub fn hash_code(&self) -> u64 {
        self.key().hash_code()
    }
}

impl<Client: BlockClient> BlockEntry for Entry<Client> {
    fn frame(&self) -> Frame {
        self.store_key.frame()
    }

    fn size(&self) -> usize {
        self.end - self.start
    }
}
