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

use crate::repr::ADDRESS;
use crate::repr::FIELD_NAME;
use crate::repr::FIELD_OFFSET;
use crate::repr::TYPE_ID;
use crate::repr::TYPE_NAME;
use crate::repr::TYPE_SIZE;

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

impl InternHandle {
    /// Returns the current level flag enabled for this intern handle,
    ///
    pub fn level_flags(&self) -> LevelFlags {
        LevelFlags::from_bits_truncate(self.register_hi)
    }

    /// Converts the handle to a u64 value,
    ///
    /// **Note**: This contains the full handle value.
    ///
    pub fn as_u64(&self) -> u64 {
        uuid::Uuid::from_fields(self.link, self.register_hi, self.register_lo, &[0; 8])
            .as_u64_pair()
            .0
    }

    /// Returns the register value of the current handle,
    ///
    pub fn register(&self) -> u32 {
        let register = bytemuck::cast::<[u16; 2], u32>([self.register_lo, self.register_hi]);
    
        register
    }

    /// Returns true if the current handle is a root handle,
    /// 
    pub fn is_root(&self) -> bool {
        self.level_flags() == LevelFlags::ROOT
    }

    /// Returns true if the current handle is a node handle,
    /// 
    /// **Note** A node handle contains a non-zero link value.
    /// 
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
            let _ = prev_handle.insert(InternHandle { link: 0, register_hi: hi, register_lo: lo });
        }

        let mut current = self.clone();
        current.link = 0;
        
        (prev_handle, current)
    }

    /// Returns the resource type id,
    ///
    pub async fn resource_type_id(&self) -> Option<TypeId> {
        TYPE_ID.copy(self).await
    }

    /// Tries to return the resource type id,
    ///
    pub fn try_resource_type_id(&self) -> Option<TypeId> {
        TYPE_ID.try_copy(self)
    }

    /// Returns the resource type name,
    ///
    pub async fn resource_type_name(&self) -> Option<&'static str> {
        TYPE_NAME.copy(self).await
    }

    /// Tries to return the resource type name,
    ///
    pub fn try_resource_type_name(&self) -> Option<&'static str> {
        TYPE_NAME.try_copy(self)
    }

    /// Returns the resource type size,
    ///
    pub async fn resource_type_size(&self) -> Option<usize> {
        TYPE_SIZE.copy(self).await
    }

    /// Tries to return the resource type size,
    ///
    pub fn try_resource_type_size(&self) -> Option<usize> {
        TYPE_SIZE.try_copy(self)
    }

    /// Returns the field offset,
    ///
    pub async fn field_offset(&self) -> Option<usize> {
        FIELD_OFFSET.copy(self).await
    }

    /// Tries to return the field offset,
    ///
    pub fn try_field_offset(&self) -> Option<usize> {
        FIELD_OFFSET.try_copy(self)
    }

    /// Returns the field name,
    ///
    pub async fn field_name(&self) -> Option<&'static str> {
        FIELD_NAME.copy(self).await
    }

    /// Tries to return the field name,
    ///
    pub fn try_field_name(&self) -> Option<&'static str> {
        FIELD_NAME.try_copy(self)
    }

    /// Returns the address,
    ///
    pub async fn address(&self) -> Option<Arc<String>> {
        ADDRESS.strong_ref(self).await
    }

    /// Tries to return the address,
    ///
    pub fn try_address(&self) -> Option<Arc<String>> {
        ADDRESS.try_strong_ref(self)
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
    pub fn result(mut self) -> anyhow::Result<InternHandle> {
        if let Some(err) = self.error.take() {
            Err(err)
        } else {
            Ok(self.handle)
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
