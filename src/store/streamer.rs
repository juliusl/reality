use crate::{
    wire::{Data, Encoder, Frame, ResourceId},
    Attributes,
};
use bytes::Bytes;
pub use signature::{RandomizedSigner, Signature, Verifier};
use std::vec;
use tokio::io::AsyncWriteExt;
use tracing::{event, Level};

use crate::store::FrameStream;

/// Struct w/ channels to stream upload a store,
///
pub struct Streamer {
    /// Channel for receiving frames that need to be encoded,
    ///
    frames: FrameStream,
    /// Resource id of this streamer,
    ///
    resource_id: ResourceId,
    /// Current bytes submitted by this streamer,
    ///
    bytes_submitted: usize,
}

impl Streamer {
    /// Returns a new streamer,
    ///
    pub fn new(resource_id: ResourceId, frames: FrameStream) -> Self {
        Streamer {
            frames,
            resource_id,
            bytes_submitted: 0,
        }
    }

    /// Submits a frame,
    ///
    /// If a blob is provided, the frame will have it's extent information set now,
    /// this allows extent frames to be passed with Value::Empty initially, and w/ the actual bytes as a parameter.
    ///
    pub async fn submit_frame(&mut self, mut frame: Frame, blob: Option<Blob>) {
        if let Some(blob) = blob.as_ref() {
            let cursor = self.bytes_submitted;
            self.bytes_submitted += blob.len();

            match blob {
                Blob::Text(_) => {
                    frame = frame.set_text_extent(blob.len() as u64, cursor as u64);
                }
                Blob::Binary(_) => {
                    frame = frame.set_binary_extent(blob.len() as u64, cursor as u64);
                }
                _ => {}
            }
        }

        match self
            .frames
            .send((self.resource_id.clone(), frame.clone(), blob))
        {
            Ok(_) => {
                event!(
                    Level::TRACE,
                    "Submitted frame, {:#}\n\tfor {:?}",
                    frame,
                    self.resource_id
                );
            }
            Err(err) => {
                event!(Level::ERROR, "Could not send frame {err}");
            }
        }
    }

    /// Submits an entire encoder,
    ///
    /// Used in case there aren't really any complicated blobs being transmitted. For example, in the Filesystem case, submit_frame gives more
    /// flexibility over using just an encoder.
    ///
    pub async fn submit_encoder(&mut self, encoder: Encoder) {
        for frame in encoder.frames {
            if frame.is_extent() {
                if let Some(Data::Extent {
                    length,
                    cursor: Some(cursor),
                }) = frame.value()
                {
                    let start = cursor as usize;
                    let end = start + length as usize;
                    let blob = &encoder.blob_device.get_ref()[start..end];

                    match frame.attribute() {
                        Some(Attributes::BinaryVector) => {
                            self.submit_frame(
                                frame,
                                Some(Blob::Binary(Bytes::copy_from_slice(blob))),
                            )
                            .await;
                        }
                        Some(Attributes::Text) => {
                            self.submit_frame(
                                frame,
                                Some(Blob::Text(
                                    String::from_utf8(blob.to_vec()).expect("should be valid utf8"),
                                )),
                            )
                            .await;
                        }
                        _ => {
                            panic!("Invalid frame type")
                        }
                    }
                }
            } else {
                self.submit_frame(frame, None).await;
            }
        }
    }
}

/// Enumeration of blob types that can be streamed,
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blob {
    /// UTF8 encoded bytes,
    ///
    Text(String),
    /// Raw u8 bytes,
    ///
    Binary(Bytes),
    /// Compressed bytes,
    ///
    Compressed(Bytes),
    /// Signed blob tuple, (signed data, signature)
    ///
    Signed(Box<Self>, Bytes),
}

impl Blob {
    /// Returns the length in bytes of this blob,
    ///
    pub fn len(&self) -> usize {
        match self {
            Blob::Text(text) => text.len(),
            Blob::Binary(bin) => bin.len(),
            Blob::Compressed(c) => c.len(),
            Blob::Signed(b, sig) => b.len() + sig.len(),
        }
    }

    /// Compresses self returning the resulting blob,
    ///
    pub async fn compress(self) -> Blob {
        match self {
            Blob::Text(bin) => {
                let buf = vec![];

                let mut gz_encoder = async_compression::tokio::write::GzipEncoder::with_quality(
                    buf,
                    async_compression::Level::Fastest,
                );

                match gz_encoder.write_all(bin.as_bytes()).await {
                    Ok(_) => {
                        gz_encoder
                            .shutdown()
                            .await
                            .expect("should be able to shutdown encoder");

                        Blob::Compressed(gz_encoder.into_inner().into())
                    }
                    Err(err) => {
                        panic!("Could not compress {err}");
                    }
                }
            }
            Blob::Binary(bin) => {
                let buf = vec![];

                let mut gz_encoder = async_compression::tokio::write::GzipEncoder::with_quality(
                    buf,
                    async_compression::Level::Fastest,
                );

                match gz_encoder.write_all(bin.as_ref()).await {
                    Ok(_) => {
                        gz_encoder
                            .shutdown()
                            .await
                            .expect("should be able to shutdown encoder");

                        Blob::Compressed(gz_encoder.into_inner().into())
                    }
                    Err(err) => {
                        panic!("Could not compress {err}");
                    }
                }
            }
            Blob::Compressed(_) => self,
            Blob::Signed(_, _) => {
                unimplemented!("Cannot compress a signed blob, first compress and then sign");
            }
        }
    }

    /// Signs a blob w/ the provided signer,
    ///
    pub fn sign<S: Signature>(self, signer: &impl RandomizedSigner<S>) -> Blob {
        let rng = rand::thread_rng();

        let signature = signer.sign_with_rng(
            rng,
            match self {
                Blob::Text(ref t) => t.as_bytes(),
                Blob::Binary(ref b) | Blob::Compressed(ref b) => b.as_ref(),
                _ => unimplemented!("signing a signed blob is not implemented"),
            },
        );

        Blob::Signed(
            Box::new(self.clone()),
            Bytes::copy_from_slice(signature.as_bytes()),
        )
    }

    /// If blob is signed, unwraps and returns the source blob if the signature can be verified,
    ///
    pub fn verify<S: Signature>(self, verifier: &impl Verifier<S>) -> Option<Blob> {
        match self {
            Blob::Signed(b, ref sig) => match S::from_bytes(&sig) {
                Ok(signature) => {
                    let bytes: Bytes = (*b).clone().into();
                    match verifier.verify(&bytes, &signature) {
                        Ok(_) => Some(*b),
                        Err(err) => {
                            event!(Level::ERROR, "Could not verify signature, {err}");
                            None
                        }
                    }
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not create signature from bytes, {err}");
                    None
                }
            },
            _ => None,
        }
    }
}

impl Into<Bytes> for Blob {
    fn into(self) -> Bytes {
        match self {
            Blob::Text(text) => text.as_bytes().to_vec().into(),
            Blob::Binary(bytes) | Blob::Compressed(bytes) => bytes,
            Blob::Signed(b, _) => match *b {
                Blob::Text(b) => b.into(),
                Blob::Binary(b) | Blob::Compressed(b) => b.into(),
                _ => unimplemented!(),
            },
        }
    }
}

/// Tests signing blobs,
///
#[tokio::test]
async fn test_signing() {
    let mut rng = rand::thread_rng();
    let bits = 2048;
    let private_key = rsa::RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");

    let signing_key = rsa::pss::BlindedSigningKey::<sha2::Sha256>::new(private_key);
    let verifying_key: rsa::pss::VerifyingKey<_> = (&signing_key).into();

    let blob = Blob::Binary(b"test binary content".to_vec().into());
    let blob = blob.sign(&signing_key);
    let unwrapped = blob.verify(&verifying_key).expect("should be valid");
    assert_eq!(
        Blob::Binary(b"test binary content".to_vec().into()),
        unwrapped
    );

    let blob = Blob::Binary(b"test binary content".to_vec().into());
    let blob = blob.compress().await.sign(&signing_key);
    let unwrapped = blob.verify(&verifying_key).expect("should be valid");
    assert_eq!(
        Blob::Binary(b"test binary content".to_vec().into())
            .compress()
            .await,
        unwrapped
    );

    let blob = Blob::Text("test binary content".to_string());
    let blob = blob.sign(&signing_key);
    let unwrapped = blob.verify(&verifying_key).expect("should be valid");
    assert_eq!(Blob::Text("test binary content".to_string()), unwrapped);

    let blob = Blob::Text("test binary content".to_string());
    let blob = blob.compress().await.sign(&signing_key);
    let unwrapped = blob.verify(&verifying_key).expect("should be valid");
    assert_eq!(
        Blob::Text("test binary content".to_string())
            .compress()
            .await,
        unwrapped
    );
}
