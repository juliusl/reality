use std::path::PathBuf;

use base64::{Config, CharacterSet};
use futures::{stream::FuturesOrdered, StreamExt};
use reality::{store::StoreIndex, wire::BlockClient};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
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

impl ArchiveBlockTransport {
    /// Returns a new transport from a store index,
    ///
    pub async fn transport<Client>(
        prefix: impl AsRef<str>,
        name: impl AsRef<str>,
        index: &StoreIndex<Client>,
    ) -> Self
    where
        Client: BlockClient,
    {
        let path = PathBuf::from(prefix.as_ref());

        tokio::fs::create_dir_all(&path)
            .await
            .expect("should be able to create dirs");

        let path = path.join(name.as_ref());

        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&path)
            .await
            .expect("should be able to open file");

        let mut builder = tokio_tar::Builder::new(file);

        let mut entries = index.entries_ordered();

        let mut blocks = entries
            .drain(..)
            .enumerate()
            .map(|(idx, entry)| {
                tokio::spawn(async move {
                    let mut header = Header::new_gnu();
                    header.set_entry_type(EntryType::Block);
                    header.set_device_minor(idx as u32).expect("should be able to set minor");
                    let path = base64::encode_config(entry.key().frame().bytes(), Config::new(CharacterSet::UrlSafe, false));
                    header
                        .set_path(path)
                        .expect("should be able to set as path");
                    let mut transport = entry.transport();
                    let mut buf = vec![];
                    transport.read_to_end(&mut buf).await.expect("should be able to read to end");
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

        Self {
            client: ArchiveBlockClient::new(path),
        }
    }

    /// Returns a new block client,
    ///
    pub fn client(&self) -> impl BlockClient {
        self.client.clone()
    }
}
