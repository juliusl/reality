use std::time::Duration;

use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;
use crate::ThunkContext;

///
///
#[derive(Default, Clone)]
pub struct KvpConfig {
    /// TTL in seconds,
    ///
    pub ttl_s: Option<Duration>,
}

/// Extends the thunk context w/ a kv-store api,
///
/// **Note** Uses cache storage on the thunk context which is always exclusively owned by the thunk context.
/// Also allows the caller to use storage in non-async contexts.
///
pub trait KvpExt {
    /// Returns true if the kv store contains value P at key,
    ///
    fn kv_contains<R>(&self, key: impl std::hash::Hash) -> bool
    where
        R: Send + Sync + 'static;

    fn kv_get<R>(
        &self,
        key: ResourceKey<R>,
    ) -> Option<<Shared as StorageTarget>::BorrowResource<'_, R>>
    where
        R: Send + Sync + 'static;

    fn kv_get_mut<R>(
        &mut self,
        key: ResourceKey<R>,
    ) -> Option<<Shared as StorageTarget>::BorrowMutResource<'_, R>>
    where
        R: Send + Sync + 'static;

    fn maybe_store_kv<R>(
        &mut self,
        key: impl std::hash::Hash,
        value: R,
    ) -> (
        ResourceKey<R>,
        <Shared as StorageTarget>::BorrowMutResource<'_, R>,
    )
    where
        R: Send + Sync + 'static;

    /// Store a resource by key in cache,
    ///
    fn store_kv<R>(&mut self, key: impl std::hash::Hash, value: R)
    where
        R: Send + Sync + 'static;

    /// Takes a resource by key from the cache,
    ///
    fn take_kv<R>(&mut self, key: impl std::hash::Hash) -> Option<(ResourceKey<R>, R)>
    where
        R: Send + Sync + 'static;

    /// Deletes a resource from kv store,
    ///
    fn delete_kv<R>(&mut self, key: impl std::hash::Hash) -> Option<ResourceKey<R>>
    where
        R: Send + Sync + 'static;

    /// Fetch a kv pair by key,
    ///
    fn fetch_kv<R>(
        &self,
        key: impl std::hash::Hash,
    ) -> Option<(
        ResourceKey<R>,
        <Shared as StorageTarget>::BorrowResource<'_, R>,
    )>
    where
        R: Send + Sync + 'static;

    /// Fetch a mutable reference to a kv pair by key,
    ///
    fn fetch_mut_kv<R>(
        &mut self,
        key: impl std::hash::Hash,
    ) -> Option<(
        ResourceKey<R>,
        <Shared as StorageTarget>::BorrowMutResource<'_, R>,
    )>
    where
        R: Send + Sync + 'static;
}

impl KvpExt for ThunkContext {
    /// Returns true if the kv store contains value P at key,
    ///
    fn kv_contains<R>(&self, key: impl std::hash::Hash) -> bool
    where
        R: Send + Sync + 'static,
    {
        let key = self.attribute.transmute().branch(&key);
        self.__cached.resource::<R>(key).is_some()
    }

    fn kv_get<R>(
        &self,
        key: ResourceKey<R>,
    ) -> Option<<Shared as StorageTarget>::BorrowResource<'_, R>>
    where
        R: Send + Sync + 'static,
    {
        self.__cached.resource(key)
    }

    fn kv_get_mut<R>(
        &mut self,
        key: ResourceKey<R>,
    ) -> Option<<Shared as StorageTarget>::BorrowMutResource<'_, R>>
    where
        R: Send + Sync + 'static,
    {
        self.__cached.resource_mut(key)
    }

    fn maybe_store_kv<R>(
        &mut self,
        key: impl std::hash::Hash,
        value: R,
    ) -> (
        ResourceKey<R>,
        <Shared as StorageTarget>::BorrowMutResource<'_, R>,
    )
    where
        R: Send + Sync + 'static,
    {
        let set_value = !self.kv_contains::<R>(&key);

        if let Some((_, __config)) = self.fetch_mut_kv::<KvpConfig>(&key) {
            // TODO --
        }

        if set_value {
            // eprintln!("Initializing {}", std::any::type_name::<R>());
            self.store_kv(&key, value);
        }

        self.fetch_mut_kv(&key)
            .expect("should only be accessed once per context")
    }

    /// Store a resource by key in cache,
    ///
    fn store_kv<R>(&mut self, key: impl std::hash::Hash, value: R)
    where
        R: Send + Sync + 'static,
    {
        let key = self.attribute.transmute().branch(&key);
        self.__cached.put_resource::<R>(value, key);
    }

    /// Take the resource from the kv store,
    ///
    fn take_kv<R>(&mut self, key: impl std::hash::Hash) -> Option<(ResourceKey<R>, R)>
    where
        R: Send + Sync + 'static,
    {
        let key = self.attribute.transmute().branch(&key);
        self.__cached
            .take_resource::<R>(key)
            .map(|p| (key.expect_not_root(), *p))
    }

    /// Fetch a kv pair by key,
    ///
    fn fetch_kv<R>(
        &self,
        key: impl std::hash::Hash,
    ) -> Option<(
        ResourceKey<R>,
        <Shared as StorageTarget>::BorrowResource<'_, R>,
    )>
    where
        R: Send + Sync + 'static,
    {
        let key = self.attribute.transmute().branch(&key);
        self.__cached
            .resource::<R>(key)
            .map(|c| (key.expect_not_root(), c))
    }

    /// Deletes a resource from kv store,
    /// 
    fn delete_kv<R>(&mut self, key: impl std::hash::Hash) -> Option<ResourceKey<R>>
    where
        R: Send + Sync + 'static,
    {
        let key = self.attribute.transmute().branch(&key);

        Some(key.transmute()).filter(move |_| self.__cached.remove_resource_at(key))
    }

    /// Fetch a mutable reference to a kv pair by key,
    ///
    fn fetch_mut_kv<R>(
        &mut self,
        key: impl std::hash::Hash,
    ) -> Option<(
        ResourceKey<R>,
        <Shared as StorageTarget>::BorrowMutResource<'_, R>,
    )>
    where
        R: Send + Sync + 'static,
    {
        let key = self.attribute.transmute().branch(&key);
        self.__cached
            .resource_mut::<R>(key)
            .map(|c| (key.expect_not_root(), c))
    }
}

#[tokio::test]
async fn test_tc_cache() {
    use std::ops::Deref;
    use uuid::Uuid;

    let mut tc = ThunkContext::default();
    tc.attribute = ResourceKey::default();

    let (key, init_uuid) = tc.maybe_store_kv("hello-world", Uuid::new_v4());
    let __init_uuid = init_uuid.to_owned();
    drop(init_uuid);
    let init_uuid = __init_uuid;

    let uuid = tc.kv_get_mut(key);

    assert_eq!(init_uuid, *uuid.unwrap().deref());
    ()
}
