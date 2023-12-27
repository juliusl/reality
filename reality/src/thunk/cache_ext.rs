use crate::Shared;
use crate::StorageTarget;
use crate::ThunkContext;

/// Cache interface for the thunk context,
///
pub trait CacheExt {
    /// If cached, returns a cached value of R,
    ///
    fn cached<R: ToOwned<Owned = R> + Sync + Send + 'static>(&self) -> Option<R>;

    /// If cached, returns a referenced to the cached value,
    ///
    fn cached_ref<R: Sync + Send + 'static>(
        &self,
    ) -> Option<<Shared as StorageTarget>::BorrowResource<'_, R>>;

    /// Returns a mutable reference to a cached resource,
    ///
    fn cached_mut<R: Sync + Send + 'static>(
        &mut self,
    ) -> Option<<Shared as StorageTarget>::BorrowMutResource<'_, R>>;

    /// Writes a resource to the cache,
    ///
    fn write_cache<R: Sync + Send + 'static>(&mut self, resource: R);

    /// Takes a cached resource,
    ///
    fn take_cache<R: Sync + Send + 'static>(&mut self) -> Option<Box<R>>;

    /// Returns true if the cache was written to,
    ///
    fn maybe_write_cache<R: Sync + Send + 'static>(
        &mut self,
        resource: R,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, R>;
}

impl CacheExt for ThunkContext {
    fn cached<R: ToOwned<Owned = R> + Sync + Send + 'static>(&self) -> Option<R> {
        self.__cached
            .current_resource::<R>(self.attribute.transmute())
    }

    fn cached_ref<R: Sync + Send + 'static>(
        &self,
    ) -> Option<<Shared as StorageTarget>::BorrowResource<'_, R>> {
        self.__cached.resource::<R>(self.attribute.transmute())
    }

    fn cached_mut<R: Sync + Send + 'static>(
        &mut self,
    ) -> Option<<Shared as StorageTarget>::BorrowMutResource<'_, R>> {
        self.__cached.resource_mut::<R>(self.attribute.transmute())
    }

    fn write_cache<R: Sync + Send + 'static>(&mut self, resource: R) {
        self.__cached
            .put_resource(resource, self.attribute.transmute())
    }

    fn take_cache<R: Sync + Send + 'static>(&mut self) -> Option<Box<R>> {
        self.__cached.take_resource(self.attribute.transmute())
    }

    fn maybe_write_cache<R: Sync + Send + 'static>(
        &mut self,
        resource: R,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, R> {
        self.__cached
            .maybe_put_resource(resource, self.attribute.transmute())
    }
}
