use std::any::TypeId;

use crate::define_intern_table;
use crate::push_tag;

use crate::prelude::*;

// Intern table for resource type names
define_intern_table!(TYPE_NAME: &'static str);

// Intern table for resource type sizes
define_intern_table!(TYPE_SIZE: usize);

// Intern table for resource type ids
define_intern_table!(TYPE_ID: TypeId);

/// Resource level is the lowest level of representation,
///
/// Resource level asserts compiler information for the resource.
///
#[derive(Clone, Copy)]
pub struct ResourceLevel {
    /// Rust type id assigned by the compiler,
    ///
    type_id: Tag<TypeId>,
    /// Rust type name assigned by the compiler,
    ///
    type_name: Tag<&'static str>,
    /// Type size assigned by the compiler,
    ///
    type_size: Tag<usize>,
}

impl ResourceLevel {
    /// Creates a new type level representation,
    ///
    #[inline]
    pub fn new<T: Send + Sync + 'static>() -> Self {
        Self {
            type_id: Tag::new(&TYPE_ID, std::any::TypeId::of::<T>),
            type_name: Tag::new(&TYPE_NAME, std::any::type_name::<T>),
            type_size: Tag::new(&TYPE_SIZE, std::mem::size_of::<T>),
        }
    }
}

impl Level for ResourceLevel {
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
        push_tag!(interner, self.type_id);
        push_tag!(interner, self.type_size);
        push_tag!(interner, self.type_name);

        interner.set_level_flags(LevelFlags::ROOT);

        interner.interner()
    }

    type Mount = (TypeId, &'static str, usize);

    #[inline]
    fn mount(&self) -> Self::Mount {
        (
            self.type_id.value(),
            self.type_name.value(),
            self.type_size.value(),
        )
    }
}

/// Wrapper struct to access resource tags,
///
pub struct ResourceRepr(pub(crate) InternHandle);

impl ResourceRepr {
    /// Returns the tag value of the resource type name,
    ///
    #[inline]
    pub async fn type_name(&self) -> Option<&'static str> {
        self.0.resource_type_name().await
    }

    /// Returns the tag value of the resource type size,
    ///
    #[inline]
    pub async fn type_size(&self) -> Option<usize> {
        self.0.resource_type_size().await
    }

    /// Returns the tage value of the resource type id,
    ///
    #[inline]
    pub async fn type_id(&self) -> Option<TypeId> {
        self.0.resource_type_id().await
    }

    /// Returns the tag value of the resource type name,
    ///
    #[inline]
    pub fn try_type_name(&self) -> Option<&'static str> {
        self.0.try_resource_type_name()
    }

    /// Returns the tag value of the resource type size,
    ///
    #[inline]
    pub fn try_type_size(&self) -> Option<usize> {
        self.0.try_resource_type_size()
    }

    /// Returns the tage value of the resource type id,
    ///
    #[inline]
    pub fn try_type_id(&self) -> Option<TypeId> {
        self.0.try_resource_type_id()
    }
}
