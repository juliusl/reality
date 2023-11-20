use async_trait::async_trait;
use flexbuffers::Reader;
use reality::prelude::*;

/// Enables working with a flexbuffers root to store data,
///
#[async_trait]
pub trait FlexbufferExt {
    /// Enables a flexbuffer in the current context's cache,
    ///
    async fn enable_flexbuffer(&mut self);

    /// Write to a flexbuffer in the cache,
    ///
    async fn write_flexbuffer(
        &mut self,
        write: impl for<'a> FnOnce(&'a mut flexbuffers::Builder) + Send,
    ) -> anyhow::Result<()>;

    /// Read from a flexbuffer in the cache,
    ///
    async fn read_flexbuffer(
        &self,
        read: impl for<'a> FnMut(Reader<&'a [u8]>) + Send,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl FlexbufferExt for ThunkContext {
    async fn enable_flexbuffer(&mut self) {
        self.write_cache(flexbuffers::Builder::default());
    }

    async fn write_flexbuffer(
        &mut self,
        write: impl for<'a> FnOnce(&'a mut flexbuffers::Builder) + Send,
    ) -> anyhow::Result<()> {
        if let Some(mut builder) = self.cached_mut::<flexbuffers::Builder>() {
            write(&mut builder);
        }
        Ok(())
    }

    async fn read_flexbuffer(
        &self,
        mut read: impl for<'a> FnMut(Reader<&'a [u8]>) + Send,
    ) -> anyhow::Result<()> {
        if let Some(builder) = self.cached_ref::<flexbuffers::Builder>() {
            read(Reader::get_root(builder.view())?);
        }
        Ok(())
    }
}
