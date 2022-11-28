use tokio::io::AsyncReadExt;
use tracing::{event, Level};

use crate::wire::{BlockClient, BlockEntry, Decoder, Encoder, Frame, Interner, ControlDevice, FrameBuffer};
use std::{collections::HashMap, ops::Range, sync::Arc};

use super::{entry::Entry, StoreKey};

/// Struct for a store index,
///
/// Indexes all interned content,
///
#[derive(Clone)]
pub struct Index<Client>
where
    Client: BlockClient,
{
    /// Interner for decoding string references,
    ///
    interner: Interner,
    /// Block client for the store,
    ///
    block_client: Client,
    /// Map of byte ranges,
    ///
    map: HashMap<StoreKey, Range<usize>>,
    /// Map of blob device ranges,
    ///
    blob_device_map: HashMap<String, Vec<StoreKey>>,
}

impl<Client> Index<Client>
where
    Client: BlockClient,
{
    /// Returns a new index,
    /// 
    pub fn new(mut interner: Interner, block_client: Client) -> Self {
        interner.add_ident("");
        interner.add_ident("store");
        interner.add_ident("control");
        
        Self {
            interner,
            block_client,
            map: HashMap::default(),
            blob_device_map: HashMap::default(),
        }
    }

    /// Returns the interner,
    ///
    #[inline]
    pub fn interner(&self) -> &Interner {
        &self.interner
    }

    /// Returns the client,
    ///
    #[inline]
    pub fn client(&self) -> Client {
        self.block_client.clone()
    }

    /// Returns the blob devices for an entry,
    ///
    #[inline]
    pub fn blob_devices(&self, name: impl AsRef<str>) -> Option<Vec<StoreKey>> {
        self.blob_device_map.get(name.as_ref()).cloned()
    }

    /// Load the interner into state,
    /// 
    pub async fn load_interner(&mut self) {
        match self.block_client.list_blocks().await.expect("should be able to join") {
            Ok(resp) => {
                if let Some(entry) = resp.iter().find(|e| {
                    e.frame().name(&self.interner) == Some(String::from("store"))
                        && e.frame().symbol(&self.interner) == Some(String::from("control"))
                }) {
                    let control_device = self.block_client.stream_range(0..entry.size());
                    let interner = read_control_device(control_device).await;
                    self.interner = self.interner.merge(&interner);
                }
            }
            Err(err) => {
                event!(Level::ERROR, "Could not list blocks, {err}");
            },
        }
    }

    /// Index a block list,
    ///
    pub async fn index(&mut self) {
        let mut offset = 0;
        let mut encoder = Encoder::new();

        let block_list = self
            .block_client
            .list_blocks()
            .await
            .expect("should be able to join result")
            .expect("should be able to list blocks");

        for block in block_list.iter() {
            if let Some(frame) = self.add_entry(offset, block) {
                encoder.frames.push(frame);
            }
            offset += block.size() as usize;
        }

        encoder.interner = self.interner.clone();
        let mut decoder = Decoder::new(&encoder);

        let store_map = decoder.decode_namespace("store");
        for (symbol, decoder) in store_map.iter() {
            let mut keys = vec![];
            for frame in decoder.frames() {
                keys.push(StoreKey::new(frame.clone()));
            }

            self.blob_device_map.insert(symbol.to_string(), keys);
        }
    }

    /// Returns entries,
    ///
    pub fn entries(&self) -> impl Iterator<Item = Entry<Client>> + '_ {
        let index = self.clone();
        let index = Arc::new(index);
        let interner = self.interner.clone();
        let interner = Arc::new(interner);
        self.map.iter().map(move |(key, range)| {
            Entry::<Client>::new(index.clone(), interner.clone(), key, range.start..range.end)
        })
    }

    /// Returns entries in order,
    ///
    pub fn entries_ordered(&self) -> Vec<Entry<Client>> {
        let index = self.clone();
        let index = Arc::new(index);
        let interner = self.interner.clone();
        let interner = Arc::new(interner);
        let mut entries = self
            .map
            .iter()
            .map(move |(key, range)| {
                Entry::<Client>::new(index.clone(), interner.clone(), key, range.start..range.end)
            })
            .collect::<Vec<_>>();

        entries.sort_by(|a, b| a.start().cmp(&b.start()));

        entries
    }

    /// Returns an entry for a key,
    ///
    pub fn entry(&self, key: StoreKey) -> Option<Entry<Client>> {
        let index = self.clone();
        let index = Arc::new(index);
        let interner = self.interner.clone();
        let interner = Arc::new(interner);

        if let Some(range) = self.map.get(&key) {
            Some(Entry::<Client>::new(
                index.clone(),
                interner.clone(),
                &key,
                range.start..range.end,
            ))
        } else {
            None
        }
    }

    /// Adds an entry to the store index,
    ///
    fn add_entry(&mut self, offset: usize, block: &impl BlockEntry) -> Option<Frame> {
        let frame = block.frame();
        let key = StoreKey::new(frame.clone());

        self.map
            .insert(key, offset..(block.size() as usize) + offset);

        Some(frame)
    }
}

/// Reads a control device from reader, and returns an Interner,
/// 
pub async fn read_control_device(mut reader: impl AsyncReadExt + tokio::io::AsyncRead + Unpin,) -> Interner{
    let mut control_device = ControlDevice::default();

    let mut frame_buffer = FrameBuffer::new(100);

    let mut b = frame_buffer.next();
    while let Ok(r) = reader.read_exact(b.as_mut()).await {
        assert_eq!(r, 64);
        let frame = Frame::from(b.freeze());
        b = frame_buffer.next();

        if frame.op() == 0x00 {
            control_device.data.push(frame.clone());
        } else if frame.op() > 0x00 && frame.op() < 0x06 {
            control_device.read.push(frame.clone());
        } else if frame.op() >= 0xC1 && frame.op() <= 0xC6 {
            assert!(
                frame.op() >= 0xC1 && frame.op() <= 0xC6,
                "Index frames have a specific op code range"
            );
            control_device.index.push(frame.clone());
        }
    }

    control_device.into()
}