use std::io::{Read, Write, Seek, Cursor};

use specs::Entity;

use crate::wire::{Data, Encoder};

use super::{FrameBuilder, Frame};

/// Start of an extension frame,
///
/// When dropped will insert the extension frame before the frames that were added,
///
pub struct ExtensionToken<'a, BlobImpl = Cursor<Vec<u8>>> 
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// Frame builder,
    ///
    builder: FrameBuilder,
    /// Internal encoder,
    ///
    encoder: &'a mut Encoder<BlobImpl>,
    /// Position the extension frame will be inserted to on drop,
    ///
    insert_at: usize,
    /// Sets the entity to use for parity,
    /// 
    entity: Option<Entity>
}

impl<'a, BlobImpl> ExtensionToken<'a, BlobImpl> 
where
    BlobImpl: Read + Write + Seek + Clone + Default 
{
    /// Sets the entity to use for parity on the extension frame,
    /// 
    pub fn set_entity(&mut self, entity: Entity) {
        self.entity = Some(entity);
    }
}

impl<'a, BlobImpl> ExtensionToken<'a, BlobImpl> 
where
    BlobImpl: Read + Write + Seek + Clone + Default, 
{
    /// Creates a new token,
    /// 
    pub fn new(namespace: impl AsRef<str>, symbol: impl AsRef<str>, encoder: &'a mut Encoder<BlobImpl>) -> Self {
        let insert_at = encoder.frames.len();
        let builder = Frame::start_extension(namespace, symbol);
        Self {
            builder,
            encoder,
            insert_at,
            entity: None
        }
    }
}

impl<'a, BlobImpl> AsMut<Encoder<BlobImpl>> for ExtensionToken<'a, BlobImpl> 
where
    BlobImpl: Read + Write + Seek + Clone + Default, 
{
    fn as_mut(&mut self) -> &mut Encoder<BlobImpl> {
        &mut self.encoder
    }
}

impl<'a, BlobImpl> Drop for ExtensionToken<'a, BlobImpl> 
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    fn drop(&mut self) {
        let len = self.encoder.frames.len() - self.insert_at;

        self.builder
            .write(Data::Length(len), Some(&mut self.encoder.blob_device))
            .expect("should be able to finish frame");

        let mut frame: Frame = self.builder.cursor.clone().into();

        if let Some(entity) = self.entity.take() {
            frame.set_parity(entity);
        }

        self.encoder.frames.insert(self.insert_at, frame);
    }
}
