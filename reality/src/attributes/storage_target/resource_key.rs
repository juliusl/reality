use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Build a resource key,
///
#[derive(Clone, Default)]
pub struct ResourceKeyHashBuilder<H: Hasher + Default> {
    hasher: H,
}

impl<H: Hasher + Default> Into<ResourceKey>
    for ResourceKeyHashBuilder<H>
{
    fn into(self) -> ResourceKey {
        ResourceKey::with_hash_value(self.hasher.finish())
    }
}

impl ResourceKeyHashBuilder<DefaultHasher> {
    /// Creates a new resource key hash builder,
    ///
    pub fn new_default_hasher() -> Self {
        Self {
            hasher: DefaultHasher::new(),
        }
    }

    /// Adds to hash state,
    ///
    pub fn hash(&mut self, hashable: impl Hash) {
        hashable.hash(&mut self.hasher);
    }

    /// Finishes building the resource key w/ hash,
    ///
    pub fn finish(self) -> ResourceKey {
        self.into()
    }
}

/// Struct containing properties of a resource key,
///
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, PartialOrd)]
pub struct ResourceKey {
    /// Internal data,
    /// 
    data: u128,
}

impl ResourceKey {
    // /// Creates a new resource-key for a type,
    // ///
    // pub fn new() -> Self {

    //     uuid::Uuid::from_fields(0, 0,0, &type_key.to_ne_bytes()).into()
    // }

    /// Creates a new resource-key deriving w/ associated label,
    ///
    pub fn with_label(label: &'static str) -> Self {
        let key = label.as_ptr() as u64;

        uuid::Uuid::from_fields(0, 0, 0, &key.to_ne_bytes()).into()
    }

    /// Creates a new resource-key derived from hashable input,
    ///
    pub fn with_hash(hashable: impl std::hash::Hash) -> Self {
        let mut key = DefaultHasher::new();
        hashable.hash(&mut key);

        let key = key.finish();

        uuid::Uuid::from_fields(0, 0, 0, &key.to_ne_bytes()).into()
    }

    /// Creates a new resource_key set w/ a hash value,
    ///
    pub fn with_hash_value(hash: u64) -> Self {
        let key = hash;
        uuid::Uuid::from_fields(0, 0, 0, &key.to_ne_bytes()).into()
    }

    /// Decodes the key from data,
    ///
    pub fn key(&self) -> u64 {
        u64::from_ne_bytes(*uuid::Uuid::from_u128(self.data).as_fields().3)
    }
}

impl<H: Hasher + Default> Hasher for ResourceKeyHashBuilder<H> {
    fn finish(&self) -> u64 {
        self.hasher.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        bytes.hash(&mut self.hasher);
    }
}

impl From<uuid::Uuid> for ResourceKey {
    fn from(value: uuid::Uuid) -> Self {
        Self {
            data: value.as_u128(),
        }
    }
}

impl TryFrom<Option<&'static str>> for ResourceKey {
    type Error = Option<ResourceKey>;

    fn try_from(value: Option<&'static str>) -> Result<Self, Self::Error> {
        if let Some(value) = value {
            Ok(ResourceKey::with_label(value))
        } else {
            Err(None)
        }
    }
}

#[test]
fn test_resource_key() {
    struct Test;

    let id = std::any::TypeId::of::<Test>();
    println!("{:x}", std::ptr::addr_of!(id) as u64);
    println!("{:?}", std::any::type_name::<Test>().as_ptr());

    let key = ResourceKey::with_label("test_label");
    let key_with_label = key.key();
    // println!("{:?} {}", key.label(), key_with_label);

    let key = ResourceKey::with_hash("test_label");
    let key_with_hash = key.key();
    println!("{}", key_with_hash);

    // Even though the above keys use the same value, they should produce different keys
    assert_ne!(key_with_label, key_with_hash);
}
