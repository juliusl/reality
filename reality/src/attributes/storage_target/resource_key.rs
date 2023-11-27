use core::slice;

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::hash::Hash;
use std::marker::PhantomData;
use serde::Serialize;
use serde::Deserialize;

use super::target::StorageTargetKey;

/// Build a resource key,
///
pub struct ResourceKeyHashBuilder<T: Send + Sync + 'static, H: Hasher + Default> {
    hasher: H,
    _t: PhantomData<T>,
}

impl<T: Send + Sync + 'static, H: Hasher + Default> From<ResourceKeyHashBuilder<T, H>>
    for ResourceKey<T>
{
    fn from(val: ResourceKeyHashBuilder<T, H>) -> Self {
        ResourceKey::with_hash_value(val.hasher.finish())
    }
}

impl<T: Send + Sync + 'static> ResourceKeyHashBuilder<T, DefaultHasher> {
    /// Creates a new resource key hash builder,
    ///
    pub fn new_default_hasher() -> Self {
        Self {
            hasher: DefaultHasher::new(),
            _t: PhantomData,
        }
    }

    /// Adds to hash state,
    ///
    pub fn hash(&mut self, hashable: impl Hash) {
        hashable.hash(&mut self.hasher);
    }

    /// Finishes building the resource key w/ hash,
    ///
    pub fn finish(self) -> ResourceKey<T> {
        self.into()
    }
}

/// Struct containing properties of a resource key,
///
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd)]
pub struct ResourceKey<T: Send + Sync + 'static> {
    pub data: u128,
    #[serde(skip)]
    _t: PhantomData<T>,
}


impl<T: Send + Sync + 'static> Default for ResourceKey<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + Sync + 'static> ResourceKey<T> {
    /// Root resource key,
    /// 
    pub const fn root() -> ResourceKey<T> {
        ResourceKey { data: 0, _t: PhantomData }
    }

    /// Returns true if the current key is a root key,
    /// 
    pub const fn is_root(&self) -> bool {
        self.data == 0
    }

    /// Panics if the current key is the root key,
    /// 
    pub const fn expect_not_root(self) -> Self {
        if self.is_root() {
            panic!("Should not be root key")
        } else {
            self
        }
    }

    /// Creates a new resource-key for type,
    ///
    pub fn new() -> Self {
        let type_key = Self::type_key();
        let (sizes, flags) = Self::type_sizes();

        uuid::Uuid::from_fields(sizes, 0, flags.bits(), &type_key.to_ne_bytes()).into()
    }

    /// Creates a new resource-key derived from hashable input,
    ///
    pub fn with_hash(hashable: impl std::hash::Hash) -> Self {
        let mut key = DefaultHasher::new();
        hashable.hash(&mut key);

        let key = key.finish();
        let type_key = Self::type_key();
        let key = type_key ^ key;

        uuid::Uuid::from_fields(0, 0, ResourceKeyFlags::HASHED.bits(), &key.to_ne_bytes()).into()
    }

    /// Creates a new resource_key set w/ a hash value,
    ///
    pub fn with_hash_value(hash: u64) -> Self {
        let key = hash;
        let type_key = Self::type_key();
        let key = type_key ^ key;

        uuid::Uuid::from_fields(0, 0, ResourceKeyFlags::HASHED.bits(), &key.to_ne_bytes()).into()
    }

    /// Transmute a resource-key to a different resource key type,
    ///
    /// If a label was set, transfers the label to the new key,
    ///
    /// If hashed, transfers the hash key over,
    ///
    /// Otherwise, creates a new key.
    ///
    pub fn transmute<B: Send + Sync + 'static>(&self) -> ResourceKey<B> {
        if let Some((key, len)) = self.label_parts() {
            let bsize = std::mem::size_of::<B>();

            let (size, flags) = if bsize ^ len == 0 {
                (
                    len as u32,
                    ResourceKeyFlags::WITH_LABEL | ResourceKeyFlags::TYPE_SIZE_EQ_LABEL_LEN,
                )
            } else {
                ((bsize ^ len) as u32, ResourceKeyFlags::WITH_LABEL)
            };

            uuid::Uuid::from_fields(
                size,
                0,
                flags.bits(),
                &(ResourceKey::<B>::type_key() ^ key as u64).to_ne_bytes(),
            )
            .into()
        } else if let Some(hash) = self.hashed_parts() {
            ResourceKey::<B>::with_hash_value(hash)
        } else {
            ResourceKey::<B>::new()
        }
    }

    /// Returns the label set for this resource-key if set,
    ///
    pub fn label(&self) -> Option<&str> {
        if let Some((key, len)) = self.label_parts() {
            unsafe {
                let slice = slice::from_raw_parts(key, len);
                let str = std::str::from_utf8(slice);
                str.ok()
            }
        } else {
            None
        }
    }

    /// Returns true if this key has a cursor enabled,
    ///
    pub fn is_cursor(&self) -> bool {
        self.flags().contains(ResourceKeyFlags::ENABLE_CURSOR)
    }

    /// Creates a branch of the current resource-key,
    ///
    /// Hashes the .key() value from the current key first followed by the hash of hashable after.
    ///
    /// The result is the key returned.
    ///
    pub fn branch(&self, hashable: impl std::hash::Hash) -> Self {
        let mut hash_builder = ResourceKeyHashBuilder::<T, _>::new_default_hasher();
        hash_builder.hash(self.key());
        hash_builder.hash(hashable);

        hash_builder.finish()
    }

    /// Returns the raw label parts if they are set,
    ///
    fn label_parts(&self) -> Option<(*const u8, usize)> {
        if !self.flags().contains(ResourceKeyFlags::WITH_LABEL) {
            None
        } else {
            let key = self.key() ^ Self::type_key();

            let len = if self
                .flags()
                .contains(ResourceKeyFlags::TYPE_SIZE_EQ_LABEL_LEN)
            {
                self.sizes()
            } else {
                self.sizes() ^ std::mem::size_of::<T>()
            };

            Some((key as *const u8, len))
        }
    }

    /// Returns the hash value from the resource-key,
    ///
    fn hashed_parts(&self) -> Option<u64> {
        if !self.flags().contains(ResourceKeyFlags::HASHED) {
            None
        } else {
            let hashed = self.key() ^ Self::type_key();

            Some(hashed)
        }
    }

    /// Decodes the key from data,
    ///
    pub fn key(&self) -> u64 {
        u64::from_ne_bytes(*uuid::Uuid::from_u128(self.data).as_fields().3)
    }

    /// Returns the type sizes,
    ///
    fn type_sizes() -> (u32, ResourceKeyFlags) {
        let type_size = std::mem::size_of::<T>() as u32;
        let type_name_len = std::any::type_name::<T>().len() as u32;

        let sizes = type_size ^ type_name_len;

        if sizes == 0 {
            (type_size, ResourceKeyFlags::TYPE_SIZE_EQ_TYPE_NAME_LEN)
        } else {
            (sizes, ResourceKeyFlags::empty())
        }
    }

    /// Returns the type key value,
    ///
    fn type_key() -> u64 {
        let mut hasher = DefaultHasher::new();
        let hasher = &mut hasher;

        let type_id = std::any::TypeId::of::<T>();
        let type_name = std::any::type_name::<T>();

        type_id.hash(hasher);

        hasher.finish() ^ type_name.as_ptr() as u64
    }

    /// Decodes the resource key flags,
    ///
    fn flags(&self) -> ResourceKeyFlags {
        ResourceKeyFlags::from_bits_truncate(uuid::Uuid::from_u128(self.data).as_fields().2)
    }

    /// Decodes the sizes,
    ///
    fn sizes(&self) -> usize {
        uuid::Uuid::from_u128(self.data).as_fields().0 as usize
    }
}

impl<T: Send + Sync, H: Hasher + Default> Hasher for ResourceKeyHashBuilder<T, H> {
    fn finish(&self) -> u64 {
        self.hasher.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        bytes.hash(&mut self.hasher);
    }
}

impl<T: Send + Sync + 'static> From<uuid::Uuid> for ResourceKey<T> {
    fn from(value: uuid::Uuid) -> Self {
        Self {
            data: value.as_u128(),
            _t: PhantomData,
        }
    }
}

impl<T: Send + Sync + 'static> TryFrom<Option<&'static str>> for ResourceKey<T> {
    type Error = StorageTargetKey<T>;

    fn try_from(value: Option<&'static str>) -> Result<Self, Self::Error> {
        if let Some(value) = value {
            Ok(ResourceKey::with_hash(value))
        } else {
            Err(ResourceKey::root())
        }
    }
}

impl<T: Send + Sync + 'static> Copy for ResourceKey<T> {}

impl<T: Send + Sync + 'static> Clone for ResourceKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

bitflags::bitflags! {
    struct ResourceKeyFlags: u16 {
        /// Resource key was created with a label
        ///
        const WITH_LABEL = 1;
        /// Type size is equal to the length of the label,
        ///
        const TYPE_SIZE_EQ_LABEL_LEN = 1 << 1;
        /// Type size is equal to the length of the type name len,
        ///
        const TYPE_SIZE_EQ_TYPE_NAME_LEN = 1 << 2;
        /// Resource key was created from hashing a value,
        ///
        const HASHED = 1 << 3;
        /// Resource key will enable a cursor has a cursor enabled,
        ///
        const ENABLE_CURSOR = 1 << 4;
        /// If set, the type name of the resource has been hashed into the resource key,
        /// 
        const HASH_TYPE_NAME = 1 << 5;
        /// If set the type name of an owning type has been hashed into the resource key,
        /// 
        const HASH_OWNER_TYPE_NAME = 1 << 6;
        /// If set, the field offset has been hashed into the resource key,
        /// 
        const HASH_OFFSET_TYPE_NAME = 1 << 7;
        /// If set, the field name has been hashed into the resource key,
        /// 
        const HASH_FIELD_NAME = 1 << 8;
        /// If set, the tag value has been hashed into the resource key,
        /// 
        const HASH_TAG_VALUE = 1 << 9;
        /// If set, the length of the field name was hashed into the resoruce key,
        /// 
        const HASH_FIELD_NAME_LEN = 1 << 10;
        /// Resource key is a hashed counter, meaning within the same namespace, it can store
        /// multiple references to the same type under a hashkey plus index. This allows
        /// resource_iter to be used.
        ///
        const HASHED_COUNTER = ResourceKeyFlags::HASHED.bits() | ResourceKeyFlags::ENABLE_CURSOR.bits();
    }
}
