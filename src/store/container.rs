use std::{
    collections::{BTreeMap, HashMap},
    io::{Cursor, Read, Seek, Write}, sync::Arc,
};

use specs::shred::ResourceId;

use crate::wire::Encoder;

/// Shallow wrapper struct for containing store data,
///
pub struct Container<T, BlobImpl = Cursor<Vec<u8>>>
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// Inner container implementation,
    ///
    inner: T,
    /// Protocol for stored wire objects,
    ///
    encoders: HashMap<ResourceId, Encoder<BlobImpl>>,
    /// Index of registered object names,
    ///
    index: HashMap<ResourceId, String>,
    /// Index of registered object names,
    ///
    reverse_index: BTreeMap<String, ResourceId>,
}

impl<T, BlobImpl> Container<T, BlobImpl>
where
    BlobImpl: Read + Write + Seek + Clone + Default,
{
    /// Searches for a resource id by name,
    ///
    #[inline]
    pub fn lookup_resource_id(&self, name: impl AsRef<str>) -> Option<&ResourceId> {
        self.reverse_index.get(name.as_ref())
    }

    /// Searches for a name by resource id,
    ///
    #[inline]
    pub fn lookup_name(&self, resource_id: &ResourceId) -> Option<&String> {
        self.index.get(resource_id)
    }

    /// Searches for an encoder by resource id,
    ///
    #[inline]
    pub fn lookup_encoder(&self, resource_id: &ResourceId) -> Option<&Encoder<BlobImpl>> {
        self.encoders.get(resource_id)
    }

    /// Returns a reference to inner,
    ///
    #[inline]
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Returns a mutable reference to inner,
    ///
    #[inline]
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns a snapshot of the current container w/o the inner state,
    ///
    pub fn snapshot(&self) -> Container<(), BlobImpl> {
        Container {
            inner: (),
            encoders: self.encoders.clone(),
            index: self.index.clone(),
            reverse_index: self.reverse_index.clone(),
        }
    }
}
