use std::path::PathBuf;

use base64::{CharacterSet, Config};
use futures::{stream::FuturesOrdered, StreamExt};
use reality::{
    store::StoreIndex,
    wire::{block_tasks::TransportSource, BlockClient, BlockTransport},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tar::{EntryType, Header};

use crate::ArchiveBlockClient;

/// Wrapper struct over an archive builder, and implements bloock
/// tranporting.
///
/// A block transport must be able to transport blocks exactly from source to destination.
///
pub struct ArchiveBlockTransport {
    client: ArchiveBlockClient,
}

impl BlockTransport for ArchiveBlockTransport {
    type TransportClient = ArchiveBlockClient;

    /// Returns a new transport from a store index,
    ///
    fn transport<Client>(
        prefix: impl Into<String>,
        name: impl Into<String>,
        index: &StoreIndex<Client>,
    ) -> TransportSource<Self>
    where
        Client: BlockClient,
    {
        let prefix = prefix.into();
        let name = name.into();
        let mut entries = index.entries_ordered();
        
        tokio::spawn(async move {
            let path = PathBuf::from(prefix);

            tokio::fs::create_dir_all(&path)
                .await
                .expect("should be able to create dirs");

            let path = path.join(name);

            let file = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&path)
                .await
                .expect("should be able to open file");

            let mut builder = tokio_tar::Builder::new(file);

            let mut blocks = entries
                .drain(..)
                .enumerate()
                .map(|(idx, entry)| {
                    tokio::spawn(async move {
                        let mut header = Header::new_gnu();
                        header.set_entry_type(EntryType::Block);
                        header
                            .set_device_minor(idx as u32)
                            .expect("should be able to set minor");
                        let path = base64::encode_config(
                            entry.key().frame().bytes(),
                            Config::new(CharacterSet::UrlSafe, false),
                        );
                        header
                            .set_path(path)
                            .expect("should be able to set as path");
                        let mut transport = entry.transport();
                        let mut buf = vec![];
                        transport
                            .read_to_end(&mut buf)
                            .await
                            .expect("should be able to read to end");
                        header.set_size(buf.len() as u64);
                        header.set_cksum();

                        (header, buf)
                    })
                })
                .collect::<FuturesOrdered<_>>();

            while let Some(Ok((header, entry))) = blocks.next().await {
                builder
                    .append(&header, entry.as_ref())
                    .await
                    .expect("should be able to append");
            }

            let mut file = builder
                .into_inner()
                .await
                .expect("should be able to finish writing");

            file.shutdown().await.expect("should be able to shutdown");

            Ok(Self {
                client: ArchiveBlockClient::new(path),
            })
        })
    }

    /// Returns a new block client,
    ///
    fn client(&self) -> Self::TransportClient {
        self.client.clone()
    }
}
