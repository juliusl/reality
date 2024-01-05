use anyhow::anyhow;
use runir::prelude::FieldRepr;
use runir::prelude::HostRepr;
use runir::prelude::NodeRepr;
use runir::prelude::RecvRepr;
use runir::prelude::Repr;
use runir::prelude::ResourceRepr;
use serde::Deserialize;
use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::marker::PhantomData;
use tracing::trace;

use super::target::StorageTargetKey;
use crate::FieldPacket;
use crate::PluginRepr;

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
        ResourceKey::with_hash_key(val.hasher.finish())
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
#[derive(Serialize, Deserialize, Hash)]
pub struct ResourceKey<T: Send + Sync + 'static> {
    /// Resource key data,
    ///
    /// # Layout
    ///
    /// u32 - reserved
    /// u16 - reserved
    /// u16 - reserved
    ///
    /// [u8; 8] - key
    ///
    /// # Operations
    ///
    /// key = ty ^ hash_value
    ///
    /// transmute = key ^ ty ^ next_ty
    ///
    /// hash_value = hash(idx) + hash(tag) + hash(label)
    ///
    pub data: u128,
    #[serde(skip)]
    _t: PhantomData<T>,
}

impl<T: Send + Sync + 'static> ResourceKey<T> {
    /// Root resource key,
    ///
    pub const fn root() -> ResourceKey<T> {
        ResourceKey {
            data: 0,
            _t: PhantomData,
        }
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

        uuid::Uuid::from_fields(0, 0, 0, &type_key.to_be_bytes()).into()
    }

    /// Creates a new resource key derived from a repr,
    ///
    #[inline]
    pub fn with_repr(repr: runir::prelude::Repr) -> Self {
        let mut n = Self::with_hash_key(repr.as_u64());
        n.set_repr(repr);
        n
    }

    /// Creates a new resource-key derived from hashable input,
    ///
    pub fn with_hash(hashable: impl std::hash::Hash) -> Self {
        let mut key = DefaultHasher::new();
        hashable.hash(&mut key);

        let key = key.finish();
        let type_key = Self::type_key();
        let key = type_key ^ key;

        uuid::Uuid::from_fields(0, 0, 0, &key.to_be_bytes()).into()
    }

    /// Creates a new resource_key set w/ a hash-key value,
    ///
    pub fn with_hash_key(key: u64) -> Self {
        let type_key = Self::type_key();
        let key = type_key ^ key;

        uuid::Uuid::from_fields(0, 0, 0, &key.to_be_bytes()).into()
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
        trace!(
            from = std::any::type_name::<T>(),
            to = std::any::type_name::<B>()
        );
        let mut rk = if self.data == 0 {
            ResourceKey::<B>::new()
        } else {
            ResourceKey::<B>::with_hash_key(self.hash_key())
        };

        if let Some(repr) = self.repr() {
            rk.set_repr(repr);
        }

        rk
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

        let mut next = hash_builder.finish();
        if let Some(repr) = self.repr() {
            next.set_repr(repr);
        }
        next
    }

    /// Decodes the key from data,
    ///
    pub fn key(&self) -> u64 {
        u64::from_be_bytes(*uuid::Uuid::from_u128(self.data).as_fields().3)
    }

    /// Returns the link version of the resource key if possible,
    ///
    pub fn into_link(&self) -> Option<ResourceKey<T>> {
        self.repr().map(|r| ResourceKey::<T>::with_repr(r))
    }

    /// Sets the repr for this key,
    ///
    pub fn set_repr(&mut self, repr: runir::prelude::Repr) {
        let u = uuid::Uuid::from_u128(self.data);

        let (_, r) = u.as_u64_pair();

        let u = uuid::Uuid::from_u64_pair(repr.as_u64(), r);

        self.data = u.as_u128();
    }

    /// Returns the repr handle if it is set,
    ///
    pub fn repr(&self) -> Option<runir::prelude::Repr> {
        let u = uuid::Uuid::from_u128(self.data);

        let (l, _) = u.as_u64_pair();

        if l > 0 {
            Some(Repr::from(l))
        } else {
            None
        }
    }

    /// Tries to create an empty field packet that could represent the resource
    /// associared to the current key.
    ///
    pub fn empty_packet(&self) -> Option<FieldPacket> {
        if let Some(repr) = self.repr() {
            let mut packet = FieldPacket::default();

            if let Some(resource) = repr.as_resource() {
                if let Some(type_name) = resource.type_name() {
                    packet.data_type_name = type_name.to_string();
                }

                if let Some(type_size) = resource.type_size() {
                    packet.data_type_size = type_size;
                }
            }

            if let Some(field) = repr.as_field() {
                if let Some(owner_name) = field.owner_name() {
                    packet.owner_name = owner_name.to_string();
                }

                if let Some(name) = field.name() {
                    packet.field_name = name.to_string();
                } else if packet.owner_name.is_empty() {
                    packet.field_name = "self".to_string();
                }

                if let Some(offset) = field.offset() {
                    packet.field_offset = offset;
                }
            }

            packet.attribute_hash = Some(self.data);

            Some(packet)
        } else {
            None
        }
    }

    /// Returns the plugin repr if found,
    ///
    #[inline]
    pub fn plugin(&self) -> Option<PluginRepr> {
        self.repr().and_then(|r| PluginRepr::try_from(r).ok())
    }

    /// Returns the node repr if found,
    ///
    #[inline]
    pub fn node(&self) -> Option<NodeRepr> {
        self.repr().and_then(|r| r.as_node())
    }

    /// Returns the field repr if found,
    ///
    #[inline]
    pub fn field(&self) -> Option<FieldRepr> {
        self.repr().and_then(|r| r.as_field())
    }

    /// Returns the resource repr if found,
    ///
    #[inline]
    pub fn resource(&self) -> Option<ResourceRepr> {
        self.repr().and_then(|r| r.as_resource())
    }

    /// Returns true if the current resource key can identify as type R,
    ///
    #[inline]
    pub fn is_resource<R: 'static>(&self) -> bool {
        self.resource()
            .map(|r| r.is_type::<R>())
            .unwrap_or_default()
    }

    /// Returns the recv repr if found,
    ///
    #[inline]
    pub fn recv(&self) -> Option<RecvRepr> {
        self.repr().and_then(|r| r.as_recv())
    }

    /// Returns the node repr if found,
    ///
    #[inline]
    pub fn host(&self) -> Option<HostRepr> {
        self.repr().and_then(|r| r.as_host())
    }

    /// Returns the hash value from the resource-key,
    ///
    #[inline]
    pub(crate) fn hash_key(&self) -> u64 {
        self.key() ^ Self::type_key()
    }

    /// Returns the type key value,
    ///
    pub(crate) fn type_key() -> u64 {
        let mut hasher = DefaultHasher::new();
        let hasher = &mut hasher;

        let type_id = std::any::TypeId::of::<T>();
        let type_name = std::any::type_name::<T>();

        type_id.hash(hasher);

        hasher.finish() ^ type_name.as_ptr() as u64
    }

    /// Derives type key from repr handle,
    ///
    pub(crate) fn split_for_soft_link(&self) -> anyhow::Result<(u64, u64)> {
        let mut hasher = DefaultHasher::new();
        let hasher = &mut hasher;

        if let (Some(r), Some(resource)) = (self.repr(), self.resource()) {
            if let Some(tyid) = resource.type_id() {
                tyid.hash(hasher);
            }

            if let Some(type_name) = resource.type_name() {
                trace!("splitting {}", type_name);
                return Ok((r.as_u64(), hasher.finish() ^ type_name.as_ptr() as u64));
            }
        }

        Err(anyhow!("Could not derive type key"))
    }

    /// Returns true if the current key is a link,
    ///
    pub fn is_link(&self) -> bool {
        if let Some(repr) = self.repr() {
            repr.as_u64() == self.hash_key()
        } else {
            false
        }
    }
}

impl<T: Send + Sync + 'static> Eq for ResourceKey<T> {}

impl<T: Send + Sync + 'static> PartialEq for ResourceKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<T: Send + Sync + 'static> Ord for ResourceKey<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.data.cmp(&other.data)
    }
}

impl<T: Send + Sync + 'static> PartialOrd for ResourceKey<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.data.partial_cmp(&other.data)
    }
}

impl<T: Send + Sync + 'static> Debug for ResourceKey<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceKey")
            .field("data", &uuid::Uuid::from_u128(self.data))
            .field("_t", &self._t)
            .finish()
    }
}

impl<T: Send + Sync + 'static> Default for ResourceKey<T> {
    fn default() -> Self {
        Self::new()
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

#[tokio::test]
async fn test_set_repr() {
    use crate::Attribute;
    use runir::prelude::DependencyLevel;
    use runir::prelude::Linker;

    // TODO: Convert eprintln to asserts

    let mut rk = ResourceKey::<Attribute>::new();
    eprintln!("{:x?}", uuid::Uuid::from_u128(rk.data));

    let mut repr = Linker::new::<String>();
    repr.push_level(DependencyLevel::new("test")).unwrap();

    let repr = repr.link().await.unwrap();
    rk.set_repr(repr);
    eprintln!("{:x?}", uuid::Uuid::from_u128(rk.data));

    let repr = rk.repr();
    eprintln!("{:04x?}", repr);

    let rk = ResourceKey::<Attribute>::with_repr(repr.unwrap());
    eprintln!("{:?}", rk);

    let res = rk.resource().unwrap();
    eprintln!("{}", res.type_name().unwrap());

    ()
}
