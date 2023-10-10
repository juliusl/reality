use std::{
    collections::{hash_map::DefaultHasher, BTreeSet},
    marker::PhantomData,
};

use crate::{ResourceKey, ResourceKeyHashBuilder};

/// Struct containing options to pass to the storage target for storing/retrieving a resource,
///
pub struct ResourceStorageConfig<T: Send + Sync + 'static> {
    labels: BTreeSet<&'static str>,
    builder: ResourceKeyHashBuilder<DefaultHasher>,
    flags: ResourceStorageConfigFlags,
    _t: PhantomData<T>,
}

impl<T: Send + Sync + 'static> Clone for ResourceStorageConfig<T> {
    fn clone(&self) -> Self {
        Self {
            labels: self.labels.clone(),
            builder: self.builder.clone(),
            flags: self.flags,
            _t: self._t.clone(),
        }
    }
}

impl<T: Send + Sync + 'static> From<ResourceKey> for ResourceStorageConfig<T> {
    fn from(value: ResourceKey) -> Self {
        todo!()
    }
}

impl<T: Send + Sync + 'static> ResourceStorageConfig<T> {
    pub fn new() -> Self {
        todo!()
    }

    pub fn new_singleton() -> Self {
        Self {
            labels: BTreeSet::new(),
            builder: ResourceKeyHashBuilder::default(),
            flags: ResourceStorageConfigFlags::empty(),
            _t: PhantomData,
        }
    }

    /// Returns the variant_id if this
    ///
    pub fn variant_id(&self) -> Option<ResourceKey> {
        todo!()
    }

    /// Transmutes this type
    ///
    pub fn transmute<B: Send + Sync + 'static>(&self) -> ResourceStorageConfig<B> {
        todo!()
    }

    /// (Chainable) Adds a label to the internal set of labels,
    ///
    pub fn with_label(mut self, label: &'static str) -> Self {
        self.labels.insert(label);
        self
    }

    /// (Chainable) Adds a hash to the internal hasher,
    ///
    pub fn with_hash(mut self, hashable: impl std::hash::Hash) -> Self {
        hashable.hash(&mut self.builder);
        self
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct ResourceStorageConfigFlags: u16 {
        /// Indicates the resource being stored, should be stored as a singleton.
        ///
        const SINGLETON = 1;
    }
}
