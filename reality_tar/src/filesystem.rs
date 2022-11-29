use reality::{
    store::{
        streamer::{RandomizedSigner, Signature},
        Blob, StoreEntry, Streamer,
    },
    wire::{Frame, Interner},
    Value,
};
use std::{io::Cursor, path::PathBuf, pin::Pin};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, DuplexStream},
};
use tokio_stream::StreamExt;
use tokio_tar::{Archive, Builder, Header};
use tracing::{event, Level};

/// Struct for encoding/decoding a filesystem to/from the store,
///
/// A TAR is used to represent and encode the filesystem contents.
///
pub struct Filesystem {
    /// Root archive source,
    ///
    archive: Option<ArchiveSource>,
}

impl Filesystem {
    /// Opens a compressed archive,
    ///
    pub async fn open_tar_gz(path: impl AsRef<str>) -> Option<Self> {
        match tokio::fs::File::open(path.as_ref()).await {
            Ok(stream) => {
                let (reader, mut writer) = tokio::io::duplex(8 * 1024);

                tokio::spawn(async move {
                    let mut decoder =
                        async_compression::tokio::write::GzipDecoder::new(&mut writer);

                    let mut reader = BufReader::new(stream);

                    match tokio::io::copy_buf(&mut reader, &mut decoder).await {
                        Ok(copied) => {
                            event!(Level::TRACE, "Decoded {copied} bytes");
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Error decoding stream, {err}");
                        }
                    }

                    reader
                        .shutdown()
                        .await
                        .expect("should be able to shutdown reader");
                    decoder
                        .shutdown()
                        .await
                        .expect("should be able to shutdown the decoder");
                });

                Some(Self {
                    archive: Some(ArchiveSource::Stream(reader)),
                })
            }
            Err(err) => {
                event!(Level::ERROR, "Could not load tar.gz, {err}");
                None
            }
        }
    }

    /// Load archive from the filesystem,
    ///
    pub async fn open_tar(path: impl AsRef<str>) -> Option<Self> {
        match tokio::fs::OpenOptions::new()
            .read(true)
            .open(path.as_ref())
            .await
        {
            Ok(stream) => Some(Self {
                archive: Some(ArchiveSource::File(stream)),
            }),
            Err(_) => None,
        }
    }

    /// Returns filesystem from a streamed tar file,
    ///
    pub fn stream_tar(stream: DuplexStream) -> Self {
        Self {
            archive: Some(ArchiveSource::Stream(stream)),
        }
    }

    /// Returns an empty filesystem,
    ///
    pub fn empty() -> Self {
        Self { archive: None }
    }

    /// Consumes and returns the archive from archive source,
    ///
    pub fn take(&mut self) -> Option<Archive<impl AsyncRead + Unpin>> {
        if let Some(archive) = self.archive.take() {
            Some(Archive::new(archive))
        } else {
            None
        }
    }

    /// Writes the current archive to disk,
    ///
    pub async fn write_disk(&mut self, path: impl AsRef<str>) {
        if let Some(builder) = self.take() {
            let path = PathBuf::from(path.as_ref());

            tokio::fs::create_dir_all(&path.parent().unwrap())
                .await
                .expect("should be able to create dirs");

            match tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(path)
                .await
            {
                Ok(mut file) => match builder.into_inner() {
                    Ok(mut r) => {
                        tokio::io::copy(&mut r, &mut file)
                            .await
                            .expect("should be able to copy");
                    }
                    Err(_) => todo!(),
                },
                Err(err) => {
                    event!(Level::ERROR, "Error opening file, {err}");
                }
            }
        }
    }

    /// Unpack an archive to the specified destination,
    ///
    pub async fn unpack(&mut self, path: impl AsRef<str>) {
        if let Some(mut archive) = self.take() {
            match archive.unpack(path.as_ref()).await {
                Ok(_) => {}
                Err(err) => {
                    event!(Level::ERROR, "Could not unpack, {err}");
                }
            }
        }
    }

    /// Consumes the inner-archive and stream's w/ streamer,
    ///
    pub async fn stream(&mut self, streamer: &mut Streamer) -> Interner {
        let mut interner = Interner::default();
        interner.add_ident("tar");
        interner.add_ident("EOF");

        if let Some(mut archive) = self.take() {
            match archive.entries() {
                Ok(mut entries) => {
                    while let Some(entry) = entries.next().await {
                        match entry {
                            Ok(mut entry) => {
                                let header = entry.header();
                                let path = header
                                    .path()
                                    .expect("should be a path")
                                    .to_str()
                                    .expect("should be a string")
                                    .to_string();
                                let size = header.size().expect("should have a size");
                                interner.add_ident(&path);

                                let mut buf = entry.header().as_bytes().to_vec();
                                buf.reserve(size as usize);

                                match entry.read_to_end(&mut buf).await {
                                    Ok(_) => {
                                        let blob = Blob::Binary(buf.into());
                                        streamer
                                            .submit_frame(
                                                Frame::define(
                                                    "tar",
                                                    path,
                                                    &Value::Empty,
                                                    &mut Cursor::<[u8; 1]>::default(),
                                                ),
                                                Some(blob),
                                            )
                                            .await;
                                    }
                                    Err(err) => {
                                        event!(Level::ERROR, "Could not read entry {err}");
                                    }
                                }
                            }
                            Err(err) => {
                                event!(Level::ERROR, "Could not get next entry, {err}");
                            }
                        }
                    }
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not iterate over entries, {err}");
                }
            }

            let mut eof = vec![];
            match archive.read_to_end(&mut eof).await {
                Ok(read) => {
                    event!(Level::TRACE, "Read {read} bytes, at EOF");
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not read end of file, {err}");
                }
            }

            streamer
                .submit_frame(
                    Frame::define(
                        "tar",
                        "EOF",
                        &Value::Empty,
                        &mut Cursor::<[u8; 1]>::default(),
                    ),
                    Some(Blob::Binary(eof.into())),
                )
                .await;
        }

        interner
    }

    /// Consumes the inner-archive and stream's w/ streamer, signs each tar entry
    ///
    pub async fn stream_signed<S: Signature>(
        &mut self,
        streamer: &mut Streamer,
        signer: impl RandomizedSigner<S>,
    ) -> Interner {
        let mut interner = Interner::default();
        interner.add_ident("tar");
        interner.add_ident("EOF");

        if let Some(mut archive) = self.take() {
            match archive.entries() {
                Ok(mut entries) => {
                    while let Some(entry) = entries.next().await {
                        match entry {
                            Ok(mut entry) => {
                                let header = entry.header();
                                let path = header
                                    .path()
                                    .expect("should be a path")
                                    .to_str()
                                    .expect("should be a string")
                                    .to_string();
                                let size = header.size().expect("should have a size");
                                interner.add_ident(&path);

                                let mut buf = entry.header().as_bytes().to_vec();
                                buf.reserve(size as usize);

                                match entry.read_to_end(&mut buf).await {
                                    Ok(_) => {
                                        let blob = Blob::Binary(buf.into());
                                        if let Blob::Signed(blob, signature) = blob.sign(&signer) {
                                            let signature = Blob::Binary(signature);
                                            streamer
                                                .submit_frame(
                                                    Frame::define(
                                                        "signature",
                                                        &path,
                                                        &Value::Empty,
                                                        &mut Cursor::<[u8; 1]>::default(),
                                                    ),
                                                    Some(signature),
                                                )
                                                .await;

                                            streamer
                                                .submit_frame(
                                                    Frame::define(
                                                        "tar",
                                                        path,
                                                        &Value::Empty,
                                                        &mut Cursor::<[u8; 1]>::default(),
                                                    ),
                                                    Some(*blob),
                                                )
                                                .await;
                                        } else {
                                            panic!("expected signature")
                                        }
                                    }
                                    Err(err) => {
                                        event!(Level::ERROR, "Could not read entry {err}");
                                    }
                                }
                            }
                            Err(err) => {
                                event!(Level::ERROR, "Could not get next entry, {err}");
                            }
                        }
                    }
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not iterate over entries, {err}");
                }
            }

            let mut eof = vec![];
            match archive.read_to_end(&mut eof).await {
                Ok(read) => {
                    event!(Level::TRACE, "Read {read} bytes, at EOF");
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not read end of file, {err}");
                }
            }

            streamer
                .submit_frame(
                    Frame::define(
                        "tar",
                        "EOF",
                        &Value::Empty,
                        &mut Cursor::<[u8; 1]>::default(),
                    ),
                    Some(Blob::Binary(eof.into())),
                )
                .await;
        }

        interner
    }

    /// Writes an archive to writer, w/ the parent fs entry
    ///
    pub async fn write_to<
        Client: reality::wire::BlockClient,
        W: AsyncWrite + Unpin + Send + 'static,
    >(
        parent_entry: &StoreEntry<Client>,
        writer: W,
    ) -> std::io::Result<()> {
        let builder = Builder::new(writer);

        let builder = parent_entry
            .join_blob_device(builder, |mut builder, entry, mut blob| async move {
                if entry.symbol() != Some(&String::from("EOF")) {
                    let header = Header::from_byte_slice(&blob[..512]);

                    match builder.append(header, &blob.clone()[512..]).await {
                        Ok(_) => {}
                        Err(err) => {
                            event!(Level::ERROR, "Error appending entry to builder, {err}");
                        }
                    }
                } else {
                    // This is a weird case where the EOF footer from the original extends 1 block further than expected
                    // This is a naive attempt to patch that, since 2 x 512 blocks will be applied on drop(builder) by removing 1 x 512 block of zeros
                    if blob.len() > 1024 {
                        blob.truncate(blob.len() - 512);
                    }

                    builder
                        .get_mut()
                        .write_all(&blob)
                        .await
                        .expect("should be able to write end");
                }
                builder
            })
            .await;

        let mut builder = builder
            .into_inner()
            .await
            .expect("should be able to get inner");
        builder.shutdown().await
    }
}

/// Enumeration of archive sources,
///
enum ArchiveSource {
    /// Archive source from a stream,
    ///
    Stream(DuplexStream),
    /// Archive sourced from a file,
    ///
    File(File),
}

impl AsyncRead for ArchiveSource {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            ArchiveSource::Stream(stream) => {
                let stream = Pin::new(stream);

                stream.poll_read(cx, buf)
            }
            ArchiveSource::File(file) => {
                let stream = Pin::new(file);

                stream.poll_read(cx, buf)
            }
        }
    }
}

