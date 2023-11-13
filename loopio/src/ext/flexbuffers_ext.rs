use async_trait::async_trait;
use flexbuffers::Reader;
use reality::{DerefMut, StorageTarget, ThunkContext};

/// Enables working with a flexbuffers root to store data,
///
#[async_trait]
pub trait FlexbufferExt {
    /// Enables a flexbuffer on both node and cache storage,
    /// 
    async fn enable_flexbuffer_all(&mut self);

    /// Enables a flexbuffer on node storage,
    ///
    async fn enable_flexbuffer_node(&mut self);

    /// Write to a flexbuffer in node storage,
    ///
    async fn write_flexbuffer_node(
        &mut self,
        write: impl for<'a> FnOnce(&'a mut flexbuffers::Builder) + Send,
    ) -> anyhow::Result<()>;

    /// Read from a flexbuffer in node storage,
    ///
    async fn read_flexbuffer_node(
        &self,
        read: impl for<'a> FnMut(Reader<&'a [u8]>) + Send
    ) -> anyhow::Result<()>;

    /// Enables a flexbuffer in the cache,
    ///
    async fn enable_flexbuffer_cache(&mut self);

    /// Write to a flexbuffer in the cache,
    ///
    async fn write_flexbuffer_cache(
        &mut self,
        write: impl for<'a> FnOnce(&'a mut flexbuffers::Builder) + Send,
    ) -> anyhow::Result<()>;

    /// Read from a flexbuffer in the cache,
    ///
    async fn read_flexbuffer_cache(
        &self,
        read: impl for<'a> FnMut(Reader<&'a [u8]>) + Send,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl FlexbufferExt for ThunkContext {
    async fn enable_flexbuffer_all(&mut self) {
        self.enable_flexbuffer_node().await;
        self.enable_flexbuffer_cache().await;
    }

    async fn enable_flexbuffer_node(&mut self) {
        let mut node = unsafe { self.node_mut().await };

        node.put_resource(flexbuffers::Builder::default(), None);
    }

    async fn write_flexbuffer_node(
        &mut self,
        write: impl for<'a> FnOnce(&'a mut flexbuffers::Builder) + Send,
    ) -> anyhow::Result<()> {
        let mut node = unsafe { self.node_mut().await };

        if let Some(mut builder) = node.resource_mut::<flexbuffers::Builder>(None) {
            (write)(builder.deref_mut());
        }

        Ok(())
    }

    async fn read_flexbuffer_node(
        &self,
        mut read: impl for<'a> FnMut(Reader<&'a [u8]>) + Send,
    ) -> anyhow::Result<()> {
        let node = self.node().await;

        if let Some(builder) = node.resource::<flexbuffers::Builder>(None) {
            let reader = flexbuffers::Reader::get_root(builder.view())?;

            read(reader)
        }

        Ok(())
    }
    
    async fn enable_flexbuffer_cache(&mut self) {
        self.write_cache(flexbuffers::Builder::default());
    }

    async fn write_flexbuffer_cache(
        &mut self,
        write: impl for<'a> FnOnce(&'a mut flexbuffers::Builder) + Send,
    ) -> anyhow::Result<()> {
        if let Some(mut builder) = self.cached_mut::<flexbuffers::Builder>() {
            write(&mut builder);
        }
        Ok(())
    }

    async fn read_flexbuffer_cache(
        &self,
        mut read: impl for<'a> FnMut(Reader<&'a [u8]>) + Send,
    ) -> anyhow::Result<()> {
        if let Some(builder) = self.cached_ref::<flexbuffers::Builder>() {
            read(Reader::get_root(builder.view())?);
        }
        Ok(())
    }
}
