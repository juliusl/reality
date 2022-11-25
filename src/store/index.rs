use crate::wire::{BlockClient, BlockEntry, Decoder, Encoder, Frame, Interner};
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
