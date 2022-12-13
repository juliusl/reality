use std::{
    collections::HashSet,
    future::IntoFuture,
    io::{Cursor, Read, Seek, Write},
};

use crate::{
    store::StoreContainer,
    wire::{BlockBuilder, BlockStore, BlockStoreBuilder, Encoder, Frame, Interner, ResourceId},
};
use futures::Future;
use tokio::{select, task::JoinSet};
use tracing::{event, Level};

use super::{Blob, Streamer};

/// Type alias for frame being streamed
///
type StreamFrame = (ResourceId, Frame, Option<Blob>);

/// Type alias for the receiving end of the streams,
///
type FrameStreamRecv = tokio::sync::mpsc::UnboundedReceiver<StreamFrame>;

/// Type alias for sending end of frame streams,
///
pub type FrameStream = tokio::sync::mpsc::UnboundedSender<StreamFrame>;

/// Struct to represent a streaming upload of the store,
///
pub struct Stream<StoreImpl, BlobImpl = Cursor<Vec<u8>>>
where
    StoreImpl: BlockStore,
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// Recv of frames to publish,
    ///
    rx: FrameStreamRecv,
    /// Sender of frames,
    ///
    tx: FrameStream,
    /// Store that is being streamed,
    ///
    container: StoreContainer<(), BlobImpl>,
    /// Interner that will be uploaded on completion,
    ///
    interner: Interner,
    /// Block store implementation,
    ///
    block_store: StoreImpl,
}

impl<StoreImpl, BlobImpl> Stream<StoreImpl, BlobImpl>
where
    StoreImpl: BlockStore,
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// Returns a new store stream,
    /// 
    pub fn new(container: StoreContainer<(), BlobImpl>, block_store: StoreImpl) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<(ResourceId, Frame, Option<Blob>)>();
        Self {
            rx,
            tx,
            container,
            interner: Interner::default(),
            block_store,
        }
    }

    /// Returns a new streamer w/ resource_id
    ///
    pub fn streamer(&self, resource_id: ResourceId) -> Streamer {
        Streamer::new(resource_id, self.tx.clone())
    }

    /// Returns a new streamer w/ resource_id looking up the id by name from the reverse index,
    ///
    pub fn find_streamer(&self, name: impl AsRef<str>) -> Option<Streamer> {
        if let Some(id) = self.container.lookup_resource_id(name.as_ref()) {
            Some(self.streamer(id.clone()))
        } else {
            None
        }
    }

    /// Starts the stream, calls select on all registered encoders, if a future is returned, then it will be added to a joinset to await completion,
    ///
    /// When all tasks in the joinset complete, the upload will be finalized
    ///
    pub async fn start<F>(mut self, select: impl Fn(&ResourceId, &Encoder<BlobImpl>, &Stream<StoreImpl, BlobImpl>) -> Option<F> + 'static) 
    where
        F: Future<Output = Option<Interner>> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();

        let mut join_set = JoinSet::<Option<Interner>>::new();

        for (id, enc) in self.container.encoders.iter() {
            if let Some(future) = select(id, enc, &self) {
                join_set.spawn(future);
            }
        }
        let future = self.begin_streaming(rx);

        tokio::spawn(async move {
            let mut interner = Interner::default();
            while let Some(result) = join_set.join_next().await {
                event!(Level::TRACE, "Completed stream task");
                match result {
                    Ok(Some(_interner)) => {
                        interner = interner.merge(&_interner);
                    }
                    Ok(None) => {
                        event!(Level::WARN, "Stream skipped");
                    }
                    Err(err) => {
                        event!(Level::ERROR, "Could not join next streaming result, {err}");
                    }
                }
            }
            match tx.send(interner) {
                Ok(_) => {
                    event!(Level::TRACE, "Completing stream");
                }
                Err(_) => {
                    event!(Level::ERROR, "Could not complete stream");
                }
            }
        });

        future.await;
    }

    /// Try to receive the next messsage,
    ///
    async fn try_receive(
        &mut self,
        finished: &mut Option<tokio::sync::oneshot::Receiver<Interner>>,
    ) -> Option<StreamFrame> {
        if let Some(f) = finished.as_mut() {
            select! {
                next = self.rx.recv() => {
                    next
                }
                interner = f => {
                    match interner {
                        Ok(interner) => {
                            self.interner = self.interner.merge(&interner);
                        },
                        Err(err) => {
                            event!(Level::ERROR, "Error trying to receive interner, {err}");
                        },
                    }
                    finished.take();
                    self.rx.close();
                    self.rx.recv().await
                }
            }
        } else {
            self.rx.recv().await
        }
    }

    /// Begin receiving frames for stream uploading,
    ///
    async fn begin_streaming(&mut self, finished: tokio::sync::oneshot::Receiver<Interner>) {
        let mut upload_block_futures = vec![];
        let mut encoding = HashSet::<ResourceId>::default();

        let mut finished = Some(finished);

        let mut builder = self.block_store.builder().expect("should be able to build");
        while let Some((resource_id, frame, mut blob)) = self.try_receive(&mut finished).await {
            encoding.insert(resource_id.clone());

            if let Some(name) = self.container.lookup_name(&resource_id) {
                let builder = builder.build_block(name);

                if let Some(blob) = blob.take() {
                    event!(Level::TRACE, "got blob w {} bytes", blob.len());
                    let blob = blob.compress().await;
                    event!(Level::TRACE, "compressed blob to {} bytes", blob.len());
                    let put_block = builder.put_block(&frame, blob);
                    upload_block_futures.push(put_block.into_future());
                } else {
                    builder.put_frame(&frame);
                }
            } else {
                event!(
                    Level::WARN,
                    "Unregistered object type is trying to stream w/ this store stream"
                )
            }
        }

        let mut interner = self.interner.clone();
        interner.add_ident("store");
        interner.add_ident("control");
        interner.add_ident("");

        for r in encoding.drain() {
            if let Some(encoder) = self.container.lookup_encoder(&r) {
                interner = interner.merge(&encoder.interner);
            }
        }

        builder.include_interner(&interner);

        if let Ok(_store) = builder
            .finish(None::<Vec<&'static str>>)
            .await
            .expect("can join task")
        {}
    }
}
