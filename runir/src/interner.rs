use std::any::TypeId;
use std::collections::BTreeMap;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Weak;

use anyhow::anyhow;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Notify;

use crate::prelude::Repr;

/// This trait is based on the concept of string interning where the
/// goal is to store distinct string values.
///
/// This trait applies that same concept to active references to software
/// resources. It is used to define the behavior when dealing w/ resource keys
/// assigned to resources in the storage target.
///
pub trait InternerFactory {
    /// Pushes a tag to the current interner state,
    ///
    fn push_tag<T: Hash + Send + Sync + 'static>(
        &mut self,
        value: T,
        assign: impl FnOnce(InternHandle) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>
            + Send
            + 'static,
    );

    /// Sets the current level flags for the interner,
    ///
    /// **Note**: The flag should be cleared when interner is called
    ///
    fn set_level_flags(&mut self, flags: LevelFlags);

    /// Finishes generating the current intern handle and consumes the current stack of tags,
    ///
    fn interner(&mut self) -> InternResult;
}

/// Handle which can be converted into a 64-bit key,
///
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct InternHandle {
    /// Link value,
    ///
    pub(crate) link: u32,
    /// Upper register,
    ///
    /// **Note on CrcInterner impl**: The first half of the upper register contains the level bits.
    ///
    pub(crate) register_hi: u16,
    /// Lower register,
    ///
    pub(crate) register_lo: u16,
}

impl From<u64> for InternHandle {
    fn from(value: u64) -> Self {
        let u = uuid::Uuid::from_u64_pair(value, 0);

        let (link, register_hi, register_lo, _) = u.as_fields();

        Self {
            link,
            register_hi,
            register_lo,
        }
    }
}

impl InternHandle {
    /// Returns the current level flag enabled for this intern handle,
    ///
    #[inline]
    pub fn level_flags(&self) -> LevelFlags {
        LevelFlags::from_bits_truncate(self.register_hi)
    }

    /// Converts the handle to a u64 value,
    ///
    /// **Note**: This contains the full handle value.
    ///
    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.as_uuid().as_u64_pair().0
    }

    /// Returns as a uuid,
    ///
    #[inline]
    pub fn as_uuid(&self) -> uuid::Uuid {
        uuid::Uuid::from_fields(self.link, self.register_hi, self.register_lo, &[0; 8])
    }

    /// Returns the register value of the current handle,
    ///
    #[inline]
    pub fn register(&self) -> u32 {
        let register = bytemuck::cast::<[u16; 2], u32>([self.register_lo, self.register_hi]);

        register
    }

    /// Returns true if the current handle is a root handle,
    ///
    #[inline]
    pub fn is_root(&self) -> bool {
        self.level_flags() == LevelFlags::ROOT
    }

    /// Returns true if the current handle is a node handle,
    ///
    /// **Note** A node handle contains a non-zero link value.
    ///
    #[inline]
    pub fn is_node(&self) -> bool {
        self.link > 0
    }

    /// Returns a split view of the current intern handle providing the current and previous nodes,
    ///
    pub fn node(&self) -> (Option<InternHandle>, InternHandle) {
        let prev = self.link ^ self.register();

        let [lo, hi] = bytemuck::cast::<u32, [u16; 2]>(prev);

        let prev_level = LevelFlags::from_bits_truncate(hi);

        let mut prev_handle = None;
        if prev_level.bits() << 1 == self.level_flags().bits() {
            let _ = prev_handle.insert(InternHandle {
                link: 0,
                register_hi: hi,
                register_lo: lo,
            });
        }

        let mut current = self.clone();
        current.link = 0;

        (prev_handle, current)
    }

    /// Returns the resource type id,
    ///
    #[inline]
    pub async fn resource_type_id(&self) -> Option<TypeId> {
        crate::repr::resource::TYPE_ID.copy(self).await
    }

    /// Tries to return the resource type id,
    ///
    #[inline]
    pub fn try_resource_type_id(&self) -> Option<TypeId> {
        crate::repr::resource::TYPE_ID.try_copy(self)
    }

    /// Returns the resource type name,
    ///
    #[inline]
    pub async fn resource_type_name(&self) -> Option<&'static str> {
        crate::repr::resource::TYPE_NAME.copy(self).await
    }

    /// Tries to return the resource type name,
    ///
    #[inline]
    pub fn try_resource_type_name(&self) -> Option<&'static str> {
        crate::repr::resource::TYPE_NAME.try_copy(self)
    }

    /// Returns the resource type size,
    ///
    #[inline]
    pub async fn resource_type_size(&self) -> Option<usize> {
        crate::repr::resource::TYPE_SIZE.copy(self).await
    }

    /// Tries to return the resource type size,
    ///
    #[inline]
    pub fn try_resource_type_size(&self) -> Option<usize> {
        crate::repr::resource::TYPE_SIZE.try_copy(self)
    }

    /// Returns the resource parse type name,
    ///
    #[inline]
    pub async fn resource_parse_type_name(&self) -> Option<&'static str> {
        crate::repr::resource::PARSE_TYPE_NAME.copy(self).await
    }

    /// Tries to return the resource parse type name,
    ///
    #[inline]
    pub fn try_resource_parse_type_name(&self) -> Option<&'static str> {
        crate::repr::resource::PARSE_TYPE_NAME.try_copy(self)
    }
    
    /// Returns the parent of the dependency,
    ///
    #[inline]
    pub async fn dependency_parent(&self) -> Option<Repr> {
        crate::repr::dependency::DEPENDENCY_PARENT.copy(self).await
    }

    /// Tries to return the parent of the dependency,
    ///
    #[inline]
    pub fn try_dependency_parent(&self) -> Option<Repr> {
        crate::repr::dependency::DEPENDENCY_PARENT.try_copy(self)
    }

    /// Returns the name of the dependency,
    ///
    #[inline]
    pub async fn dependency_name(&self) -> Option<Arc<String>> {
        crate::repr::dependency::DEPENDENCY_NAME
            .strong_ref(self)
            .await
    }

    /// Tries to return the name of the dependency,
    ///
    #[inline]
    pub fn try_dependency_name(&self) -> Option<Arc<String>> {
        crate::repr::dependency::DEPENDENCY_NAME.try_strong_ref(self)
    }

    /// Returns the name of the receiver,
    ///
    #[inline]
    pub async fn recv_name(&self) -> Option<Arc<String>> {
        crate::repr::recv::RECV_NAMES.strong_ref(self).await
    }

    /// Tries to return the name of the receiver,
    ///
    #[inline]
    pub fn try_recv_name(&self) -> Option<Arc<String>> {
        crate::repr::recv::RECV_NAMES.try_strong_ref(self)
    }

    /// Returns the name of the receiver fields,
    ///
    #[inline]
    pub async fn recv_fields(&self) -> Option<Arc<Vec<Repr>>> {
        crate::repr::recv::RECV_FIELDS.strong_ref(self).await
    }

    /// Tries to return the receiver fields,
    ///
    #[inline]
    pub fn try_recv_fields(&self) -> Option<Arc<Vec<Repr>>> {
        crate::repr::recv::RECV_FIELDS.try_strong_ref(self)
    }

    /// Returns the type id of the owner of this field,
    ///
    #[inline]
    pub async fn owner_type_id(&self) -> Option<TypeId> {
        crate::repr::field::OWNER_ID.copy(self).await
    }

    /// Tries to return the type id of the owner of this field,
    ///
    #[inline]
    pub fn try_owner_type_id(&self) -> Option<TypeId> {
        crate::repr::field::OWNER_ID.try_copy(self)
    }

    /// Returns the type name of the owner of this field,
    ///
    #[inline]
    pub async fn owner_name(&self) -> Option<&'static str> {
        crate::repr::field::OWNER_NAME.copy(self).await
    }

    /// Tries to return the type name of the owner of this field,
    ///
    #[inline]
    pub fn try_owner_name(&self) -> Option<&'static str> {
        crate::repr::field::OWNER_NAME.try_copy(self)
    }

    /// Returns the type size of the owner of this field,
    ///
    #[inline]
    pub async fn owner_size(&self) -> Option<usize> {
        crate::repr::field::OWNER_SIZE.copy(self).await
    }

    /// Tries to return the type size of the owner of this field,
    ///
    #[inline]
    pub fn try_owner_size(&self) -> Option<usize> {
        crate::repr::field::OWNER_SIZE.try_copy(self)
    }

    /// Returns the field offset,
    ///
    #[inline]
    pub async fn field_offset(&self) -> Option<usize> {
        crate::repr::field::FIELD_OFFSET.copy(self).await
    }

    /// Tries to return the field offset,
    ///
    #[inline]
    pub fn try_field_offset(&self) -> Option<usize> {
        crate::repr::field::FIELD_OFFSET.try_copy(self)
    }

    /// Returns the field name,
    ///
    #[inline]
    pub async fn field_name(&self) -> Option<&'static str> {
        crate::repr::field::FIELD_NAME.copy(self).await
    }

    /// Tries to return the field name,
    ///
    #[inline]
    pub fn try_field_name(&self) -> Option<&'static str> {
        crate::repr::field::FIELD_NAME.try_copy(self)
    }

    /// Returns the node symbol,
    ///
    #[inline]
    pub async fn symbol(&self) -> Option<Arc<String>> {
        crate::repr::node::SYMBOL.strong_ref(self).await
    }

    /// Tries to return the node symbol,
    ///
    #[inline]
    pub fn try_symbol(&self) -> Option<Arc<String>> {
        crate::repr::node::SYMBOL.try_strong_ref(self)
    }

    /// Returns a strong reference to the input,
    ///
    #[inline]
    pub async fn input(&self) -> Option<Arc<String>> {
        crate::repr::node::INPUT.strong_ref(self).await
    }

    /// Tries to return a strong reference to the input,
    ///
    #[inline]
    pub fn try_input(&self) -> Option<Arc<String>> {
        crate::repr::node::INPUT.try_strong_ref(self)
    }

    /// Returns a strong reference to the tag,
    ///
    #[inline]
    pub async fn tag(&self) -> Option<Arc<String>> {
        crate::repr::node::TAG.strong_ref(self).await
    }

    /// Tries to return a strong reference to the tag,
    ///
    #[inline]
    pub fn try_tag(&self) -> Option<Arc<String>> {
        crate::repr::node::TAG.try_strong_ref(self)
    }

    /// Returns a strong reference to the path,
    ///
    #[inline]
    pub async fn path(&self) -> Option<Arc<String>> {
        crate::repr::node::PATH.strong_ref(self).await
    }

    /// Tries to return a strong reference to the path,
    ///
    #[inline]
    pub fn try_path(&self) -> Option<Arc<String>> {
        crate::repr::node::PATH.try_strong_ref(self)
    }

    /// Returns a strong reference to the node idx,
    ///
    #[inline]
    pub async fn node_idx(&self) -> Option<usize> {
        crate::repr::node::NODE_IDX.copy(self).await
    }

    /// Tries to return a strong reference to the node idx,
    ///
    #[inline]
    pub fn try_node_idx(&self) -> Option<usize> {
        crate::repr::node::NODE_IDX.try_copy(self)
    }

    /// Returns a strong reference to node source,
    ///
    #[inline]
    pub async fn node_source(&self) -> Option<Arc<String>> {
        crate::repr::node::SOURCE.strong_ref(self).await
    }

    /// Tries to return a strong reference to node source,
    ///
    #[inline]
    pub fn try_node_source(&self) -> Option<Arc<String>> {
        crate::repr::node::SOURCE.try_strong_ref(self)
    }

    /// Returns a strong reference to doc_headers,
    ///
    #[inline]
    pub async fn doc_headers(&self) -> Option<Arc<Vec<String>>> {
        crate::repr::node::DOC_HEADERS.strong_ref(self).await
    }

    /// Tries to return a strong reference to doc_headers,
    ///
    #[inline]
    pub fn try_doc_headers(&self) -> Option<Arc<Vec<String>>> {
        crate::repr::node::DOC_HEADERS.try_strong_ref(self)
    }

    /// Returns a strong reference to annotations,
    ///
    #[inline]
    pub async fn annotations(&self) -> Option<Arc<BTreeMap<String, String>>> {
        crate::repr::node::ANNOTATIONS.strong_ref(self).await
    }

    /// Tries to return a strong reference to annotations,
    ///
    #[inline]
    pub fn try_annotations(&self) -> Option<Arc<BTreeMap<String, String>>> {
        crate::repr::node::ANNOTATIONS.try_strong_ref(self)
    }

    /// Returns the host address,
    ///
    #[inline]
    pub async fn host_address(&self) -> Option<Arc<String>> {
        crate::repr::host::ADDRESS.strong_ref(self).await
    }

    /// Tries to return the host address,
    ///
    #[inline]
    pub fn try_host_address(&self) -> Option<Arc<String>> {
        crate::repr::host::ADDRESS.try_strong_ref(self)
    }

    /// Returns extensions added to this host,
    ///
    #[inline]
    pub async fn host_extensions(&self) -> Option<Arc<Vec<Repr>>> {
        crate::repr::host::EXTENSIONS.strong_ref(self).await
    }

    /// Tries to return extensions added to this host,
    ///
    #[inline]
    pub fn try_host_extensions(&self) -> Option<Arc<Vec<Repr>>> {
        crate::repr::host::EXTENSIONS.try_strong_ref(self)
    }
}

/// Return type that can be notified when the handle is ready for use,
///
pub struct InternResult {
    /// The inner intern handle,
    ///
    pub(crate) handle: InternHandle,

    /// Notifies when the intern handle is ready,
    ///
    pub(crate) ready: Arc<Notify>,

    /// If an error occurred this will be set,
    ///
    pub(crate) error: Option<anyhow::Error>,
}

impl InternResult {
    /// Waits for the intern handle to be ready,
    ///
    pub async fn wait_for_ready(self) -> InternHandle {
        self.ready.notified().await;
        self.handle
    }

    /// Returns as std result,
    ///
    /// **Note** Since this is not waiting for the intern handle to be ready, it's possible trying to use this handle
    /// will not return anything.
    ///
    pub fn result(mut self) -> anyhow::Result<(Arc<Notify>, InternHandle)> {
        if let Some(err) = self.error.take() {
            Err(err)
        } else {
            Ok((self.ready, self.handle))
        }
    }
}

/// Struct maintaining an inner shared intern table,
///
#[derive(Default)]
pub struct InternTable<T: Send + Sync + 'static> {
    /// Inner table,
    ///
    inner: tokio::sync::RwLock<BTreeMap<InternHandle, Arc<T>>>,
}

impl<T: Send + Sync + 'static> InternTable<T> {
    /// Creates a new empty intern table,
    ///
    #[inline]
    pub const fn new() -> Self {
        Self {
            inner: tokio::sync::RwLock::const_new(BTreeMap::new()),
        }
    }

    /// Returns a handle to the interned value,
    ///
    /// **Errors** Returns an error if the value is not currently interned, or if the
    /// inner table lock is poisoned.
    ///
    pub async fn get(&self, handle: &InternHandle) -> anyhow::Result<Weak<T>> {
        let table = self.inner.read().await;

        if let Some(value) = table.get(handle) {
            Ok(Arc::downgrade(value))
        } else {
            Err(anyhow!("Not interned {:?}", handle))
        }
    }

    /// Returns a handle to the interned value,
    ///
    /// **Errors** Returns an error if the value cannot be currently read, or if the
    /// inner table lock is poisoned.
    ///
    pub fn try_get(&self, handle: &InternHandle) -> anyhow::Result<Weak<T>> {
        let table = self.inner.try_read()?;

        if let Some(value) = table.get(handle) {
            Ok(Arc::downgrade(value))
        } else {
            Err(anyhow!("Not interned {:?}", handle))
        }
    }

    /// Returns a copy of the interned value from a handle,
    ///
    pub async fn copy(&self, handle: &InternHandle) -> Option<T>
    where
        T: Copy,
    {
        self.get(handle)
            .await
            .ok()
            .as_ref()
            .and_then(Weak::upgrade)
            .as_deref()
            .copied()
    }

    /// Tries to return a copy of the internd value from a handle,
    ///
    pub fn try_copy(&self, handle: &InternHandle) -> Option<T>
    where
        T: Copy,
    {
        self.try_get(handle)
            .ok()
            .as_ref()
            .and_then(Weak::upgrade)
            .as_deref()
            .copied()
    }

    /// Returns a clone of the interned value from a handle,
    ///
    pub async fn clone(&self, handle: &InternHandle) -> Option<T>
    where
        T: Clone,
    {
        self.get(handle)
            .await
            .ok()
            .as_ref()
            .and_then(Weak::upgrade)
            .as_deref()
            .cloned()
    }

    /// Tries to return a clone of the internd value from a handle,
    ///
    pub fn try_clone(&self, handle: &InternHandle) -> Option<T>
    where
        T: Clone,
    {
        self.try_get(handle)
            .ok()
            .as_ref()
            .and_then(Weak::upgrade)
            .as_deref()
            .cloned()
    }

    /// Returns a new strong reference to the value,
    ///
    pub async fn strong_ref(&self, handle: &InternHandle) -> Option<Arc<T>> {
        self.get(handle)
            .await
            .ok()
            .as_ref()
            .and_then(Weak::upgrade)
            .clone()
    }

    /// Tries to return a new strong reference to the value,
    ///
    pub fn try_strong_ref(&self, handle: &InternHandle) -> Option<Arc<T>> {
        self.try_get(handle)
            .ok()
            .as_ref()
            .and_then(Weak::upgrade)
            .clone()
    }

    /// Assigns an intern handle for an immutable value,
    ///
    /// **Note** If the intern handle already has been assigned a value this will result in a no-op.
    ///
    pub async fn assign_intern(&self, handle: InternHandle, value: T) -> anyhow::Result<()> {
        // Skip if the value has already been created
        {
            if self.inner.read().await.contains_key(&handle) {
                // eprintln!("Skipping interning {:?}", handle);
                return Ok(());
            }
        }

        let mut table = self.inner.write().await;

        table.insert(handle, Arc::new(value));

        Ok(())
    }
}

bitflags::bitflags! {
    /// Representation level flags,
    ///
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct LevelFlags : u16 {
        /// Root representation level
        ///
        const ROOT = 0x0100;

        /// Representation Level 1
        ///
        const LEVEL_1 = 0x0100 << 1;

        /// Representation Level 2
        ///
        const LEVEL_2 = 0x0100 << 2;

        /// Representation Level 3
        ///
        const LEVEL_3 = 0x0100 << 3;

        /// Representation level 4
        ///
        const LEVEL_4 = 0x0100 << 4;

        /// Representation level 5
        ///
        const LEVEL_5 = 0x0100 << 5;

        /// Representation level 6
        ///
        const LEVEL_6 = 0x0100 << 6;

        /// Representation level 7
        ///
        const LEVEL_7 = 0x0100 << 7;
    }
}
