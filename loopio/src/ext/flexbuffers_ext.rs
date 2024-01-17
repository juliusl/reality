use bytes::{BufMut, Bytes, BytesMut};
use flexbuffers::{Buffer, Reader};
use reality::prelude::*;
use std::ops::Deref;
use tokio::sync::RwLockReadGuard;

/// Enables working with a flexbuffers root to store data in a ThunkContext cache,
///
pub trait FlexbufferCacheExt: AsMut<ThunkContext> + AsRef<ThunkContext> {
    /// Returns a mutable reference to the flexbuffers Builder,
    ///
    /// **Note** Implicitly enables flexbuffers if not enabled already
    ///
    #[inline]
    fn flexbuffer(
        &mut self,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, flexbuffers::Builder> {
        self.as_mut()
            .maybe_write_cache(flexbuffers::Builder::default)
    }

    /// Returns a flexbuffer scope that updates the cached view on drop,
    ///
    #[inline]
    fn flexbuffer_scope(&mut self) -> FlexbufferBuilderScope<'_> {
        FlexbufferBuilderScope {
            context: self.as_mut(),
        }
    }

    /// Returns a reference to the current cached flexbuffer view,
    ///
    /// **Note** Returns None if the flexbuffers is not currently enabled.
    ///
    #[inline]
    fn flexbuffer_view(&self) -> Option<CachedFlexbufferReader<'_>> {
        self.as_ref()
            .cached_ref::<CachedFlexbufferRoot>()
            .map(|b| CachedFlexbufferRootRef::Root(RwLockReadGuard::map(b, |b| b)))
            .and_then(|b| Reader::get_root(b).ok())
    }

    /// Returns the current cached flexbuffer root as bytes,
    ///
    #[inline]
    fn flexbuffer_bytes(&self) -> Option<Bytes> {
        self.as_ref()
            .cached_ref::<CachedFlexbufferRoot>()
            .map(|c| c.0.clone())
    }

    /// Sets the current cached flexbuffer root,
    ///
    #[inline]
    fn set_flexbuffer_root(&mut self, root: Bytes) {
        self.as_mut().write_cache(CachedFlexbufferRoot(root));
    }

    /// Update the current flexbuffer view,
    ///
    #[inline]
    fn update_flexbuffer_view(&mut self) {
        let mut cached = BytesMut::new();

        if let Some(builder) = self.as_mut().cached_ref::<flexbuffers::Builder>() {
            cached.put(builder.view());
        }

        self.as_mut()
            .write_cache(CachedFlexbufferRoot(cached.freeze()));
    }
}

impl FlexbufferCacheExt for ThunkContext {}

/// Type-alias for a flex buffer reader w/ a cached root,
///
pub type CachedFlexbufferReader<'de> = flexbuffers::Reader<CachedFlexbufferRootRef<'de>>;

/// Wrapper over context for writing to a flexbuffers:Builder,
///
/// When the permit is dropped it will update the cached view.
///
pub struct FlexbufferBuilderScope<'a> {
    context: &'a mut ThunkContext,
}

impl<'a> FlexbufferBuilderScope<'a> {
    /// Returns a reference to the current builder,
    ///
    pub(crate) fn builder(
        &mut self,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, flexbuffers::Builder> {
        self.context.flexbuffer()
    }

    /// Builds a flexbuffer root,
    ///
    pub fn build(
        mut self,
        build: impl FnOnce(<Shared as StorageTarget>::BorrowMutResource<'_, flexbuffers::Builder>),
    ) {
        let builder = self.builder();
        build(builder)
    }

    /// Resets the current builder,
    ///
    pub fn reset(
        &mut self,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, flexbuffers::Builder> {
        let mut builder = self.context.flexbuffer();
        builder.reset();
        builder
    }
}

impl<'a> Drop for FlexbufferBuilderScope<'a> {
    fn drop(&mut self) {
        self.context.update_flexbuffer_view();
    }
}

/// Wrapper over cached flexbuffer data,
///
#[derive(Clone)]
pub struct CachedFlexbufferRoot(Bytes);

/// Enumeration of cache state,
///
pub enum CachedFlexbufferRootRef<'a> {
    Root(RwLockReadGuard<'a, CachedFlexbufferRoot>),
    View(CachedFlexbufferRoot),
}

impl Deref for CachedFlexbufferRoot {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<'a> Deref for CachedFlexbufferRootRef<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            CachedFlexbufferRootRef::Root(r) => &*r,
            CachedFlexbufferRootRef::View(v) => v.0.as_ref(),
        }
    }
}

impl<'a> Buffer for CachedFlexbufferRootRef<'a> {
    // TOOD -- This should've been a zero-copy implementation, but it's hard to do this w/o bloat
    type BufferString = &'a str;

    fn slice(&self, range: std::ops::Range<usize>) -> Option<Self> {
        match self {
            CachedFlexbufferRootRef::Root(r) => Some(CachedFlexbufferRootRef::View(
                CachedFlexbufferRoot(r.0.clone().slice(range)),
            )),
            CachedFlexbufferRootRef::View(r) => Some(CachedFlexbufferRootRef::View(
                CachedFlexbufferRoot(r.0.clone().slice(range)),
            )),
        }
    }

    fn empty() -> Self {
        CachedFlexbufferRootRef::View(CachedFlexbufferRoot(Bytes::new()))
    }

    fn buffer_str(&self) -> Result<Self::BufferString, std::str::Utf8Error> {
        match self {
            CachedFlexbufferRootRef::Root(r) => unsafe {
                let slice = std::slice::from_raw_parts(r.0.as_ref().as_ptr(), r.0.len());

                Ok(std::str::from_utf8(slice)?)
            },
            CachedFlexbufferRootRef::View(r) => unsafe {
                let slice = std::slice::from_raw_parts(r.0.as_ref().as_ptr(), r.0.len());

                Ok(std::str::from_utf8(slice)?)
            },
        }
    }
}

#[tokio::test]
async fn test_flexbuffers() {
    let mut context = ThunkContext::new();
    {
        let mut scope = context.flexbuffer_scope();
        let mut builder = scope.builder();
        let mut map = builder.start_map();

        map.start_map("name").push("value", "jello");
        map.start_map("name2").push("value", "jello-2");
    }

    // Test that the update was persisted on drop
    {
        let reader = context.flexbuffer_view().expect("should be enabled");
        let value = reader
            .as_map()
            .index("name")
            .ok()
            .and_then(|r| r.as_map().index("value").ok())
            .map(|v| v.as_str());
        eprintln!("{:?}", value);
        assert_eq!(Some("jello"), value);

        let value = reader
            .as_map()
            .index("name2")
            .ok()
            .and_then(|r| r.as_map().index("value").ok())
            .map(|v| v.as_str());
        eprintln!("{:?}", value);
        assert_eq!(Some("jello-2"), value);
    }

    ()
}
