use std::future::Future;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::wire::{ControlDevice, Encoder, Frame, WireObject};

use super::Protocol;

/// Async-versions of send/receive,
///
impl Protocol {
    /// Ensures and returns a mutable reference to an encoder,
    ///
    pub fn ensure_encoder<W>(&mut self) -> &mut Encoder
    where
        W: WireObject,
    {
        let resource_id = W::resource_id();

        if !self.encoders.contains_key(&resource_id) {
            self.encoders.insert(resource_id.clone(), Encoder::new());
        }

        self.encoders
            .get_mut(&resource_id)
            .expect("should have encoder")
    }

    /// Async version of Protocol::send
    ///
    pub async fn send_async<W, Writer, F>(
        &mut self,
        control_stream: impl FnOnce() -> F,
        frame_stream: impl FnOnce() -> F,
        blob_stream: impl FnOnce() -> F,
    ) where
        W: WireObject,
        Writer: AsyncWrite + Unpin,
        F: Future<Output = Writer>,
    {
        let mut control_stream = control_stream().await;
        let control_device = ControlDevice::new(self.ensure_encoder::<W>().interner.clone());
        for f in control_device.data_frames() {
            assert_eq!(control_stream.write(f.bytes()).await.ok(), Some(64))
        }

        for f in control_device.read_frames() {
            assert_eq!(control_stream.write(f.bytes()).await.ok(), Some(64));
        }

        for f in control_device.index_frames() {
            assert_eq!(control_stream.write(f.bytes()).await.ok(), Some(64));
        }

        let mut frame_stream = frame_stream().await;
        for f in self.ensure_encoder::<W>().frames_slice() {
            assert_eq!(frame_stream.write(f.bytes()).await.ok(), Some(64));
        }

        self.ensure_encoder::<W>().blob_device.set_position(0);

        let mut blob_stream = blob_stream().await;
        let blob_len = self.ensure_encoder::<W>().blob_device.get_ref().len();
        assert_eq!(
            tokio::io::copy(&mut self.ensure_encoder::<W>().blob_device, &mut blob_stream)
                .await
                .ok(),
            Some(blob_len as u64)
        );
    }

    /// Async version of Protocol receive
    ///
    pub async fn receive_async<W, Reader, F>(
        &mut self,
        control_stream: impl FnOnce() -> F,
        frame_stream: impl FnOnce() -> F,
        blob_stream: impl FnOnce() -> F,
    ) where
        W: WireObject,
        Reader: AsyncRead + Unpin,
        F: Future<Output = Reader>,
    {
        let mut control_stream = control_stream().await;
        let mut control_device = ControlDevice::default();
        let mut frame_buffer = [0; 64];
        while let Ok(64) = control_stream.read_exact(&mut frame_buffer).await {
            let frame = Frame::from(frame_buffer);
            if frame.op() == 0x00 {
                control_device.data.push(frame.clone());
            } else if frame.op() > 0x00 && frame.op() < 0x06 {
                control_device.read.push(frame.clone());
            } else {
                assert!(
                    frame.op() >= 0xC1 && frame.op() <= 0xC6,
                    "Index frames have a specific op code range"
                );
                control_device.index.push(frame.clone());
            }

            frame_buffer = [0; 64]
        }
        self.ensure_encoder::<W>().interner = control_device.into();

        let mut blob_stream = blob_stream().await;
        tokio::io::copy(&mut blob_stream, &mut self.ensure_encoder::<W>().blob_device)
            .await
            .ok();

        let mut frame_stream = frame_stream().await;
        while let Ok(64) = frame_stream.read_exact(&mut frame_buffer).await {
            let frame = Frame::from(frame_buffer);
            self.ensure_encoder::<W>().frames.push(frame);
            frame_buffer = [0; 64]
        }
        
        let encoder = self.ensure_encoder::<W>(); 
        encoder.frame_index = W::build_index(&encoder.interner, &encoder.frames);
    }
}
