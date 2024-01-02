use std::any::TypeId;
use std::fs::File;
use std::path::PathBuf;

use crate::define_intern_table;
use crate::push_tag;

use crate::prelude::*;

// Intern table for resource type names
define_intern_table!(TYPE_NAME: &'static str);

// Intern table for resource type sizes
define_intern_table!(TYPE_SIZE: usize);

// Intern table for resource type ids
define_intern_table!(TYPE_ID: TypeId);

// Intern table for resource parse type names
define_intern_table!(PARSE_TYPE_NAME: &'static str);

// Intern table for ffi type name
define_intern_table!(FFI_TYPE_NAME: &'static str);

// Intern table for ffi parse type name
define_intern_table!(FFI_PARSE_TYPE_NAME: &'static str);

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
    /// Rust type name of the type used to parse node input,
    ///
    parse_type: Option<Tag<&'static str>>,
    /// (Optional) FFI type name,
    /// 
    ffi_type: Option<Tag<&'static str>>,
    /// (Optional) FFI parse type name,
    /// 
    ffi_parse_type: Option<Tag<&'static str>>,
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
            parse_type: None,
            ffi_type: None,
            ffi_parse_type: None,
        }
    }

    /// Sets the resource parse type,
    ///
    #[inline]
    pub fn set_parse_type<T>(&mut self) {
        self.parse_type = Some(Tag::new(&PARSE_TYPE_NAME, std::any::type_name::<T>));
    }

    /// Sets the ffi type name,
    /// 
    #[inline]
    pub fn set_ffi_type<T: FFI>(&mut self) {
        self.ffi_type = Some(Tag::new(&FFI_TYPE_NAME, T::ffi_type_name));
    }

    /// Sets the ffi parse type name,
    /// 
    #[inline]
    pub fn set_ffi_parse_type<T: FFI>(&mut self) {
        self.ffi_parse_type = Some(Tag::new(&FFI_PARSE_TYPE_NAME, T::ffi_type_name));
    }
}

impl Level for ResourceLevel {
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
        push_tag!(interner, self.type_id);
        push_tag!(interner, self.type_size);
        push_tag!(interner, self.type_name);

        if let Some(parse_type) = self.parse_type {
            push_tag!(interner, parse_type);
        }

        if let Some(ffi_type_name) = self.ffi_type {
            push_tag!(interner, ffi_type_name);
        }

        if let Some(ffi_parse_type_name) = self.ffi_parse_type {
            push_tag!(interner, ffi_parse_type_name);
        }

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
    /// Returns true if resource matches type,
    ///
    pub fn is_type<T: 'static>(&self) -> bool {
        self.type_name()
            .filter(|n| *n == std::any::type_name::<T>())
            .is_some()
            && self
                .type_id()
                .filter(|n| *n == std::any::TypeId::of::<T>())
                .is_some()
    }

    /// Returns true if the resource parse type matches,
    /// 
    pub fn is_parse_type<T: 'static>(&self) -> bool {
        self.parse_type_name()
            .filter(|n| *n == std::any::type_name::<T>())
            .is_some()
    }

    /// Returns the tag value of the resource type name,
    ///
    #[inline]
    pub fn type_name(&self) -> Option<&'static str> {
        self.0.resource_type_name()
    }

    /// Returns the tag value of the resource type size,
    ///
    #[inline]
    pub fn type_size(&self) -> Option<usize> {
        self.0.resource_type_size()
    }

    /// Returns the tage value of the resource type id,
    ///
    #[inline]
    pub fn type_id(&self) -> Option<TypeId> {
        self.0.resource_type_id()
    }

    /// Returns the tag value of the resource parse type name,
    ///
    #[inline]
    pub fn parse_type_name(&self) -> Option<&'static str> {
        self.0.resource_parse_type_name()
    }
}

/// Trait to provide foreign function interface support,
/// 
pub trait FFI {
    /// FFI type name,
    /// 
    fn ffi_type_name() -> &'static str;
}

impl FFI for String {
    fn ffi_type_name() -> &'static str {
        "string"
    }
}

impl FFI for PathBuf {
    fn ffi_type_name() -> &'static str {
        "string"
    }
}

impl FFI for File {
    fn ffi_type_name() -> &'static str {
        "file"
    }
}

impl FFI for bool {
    fn ffi_type_name() -> &'static str {
        "bool"
    }
}

impl FFI for u8 {
    fn ffi_type_name() -> &'static str {
        "u8"
    }
}

impl FFI for u16 {
    fn ffi_type_name() -> &'static str {
        "u16"
    }
}

impl FFI for u32 {
    fn ffi_type_name() -> &'static str {
        "u32"
    }
}

impl FFI for u64 {
    fn ffi_type_name() -> &'static str {
        "u64"
    }
}

impl FFI for i8 {
    fn ffi_type_name() -> &'static str {
        "i8"
    }
}

impl FFI for i16 {
    fn ffi_type_name() -> &'static str {
        "i16"
    }
}

impl FFI for i32 {
    fn ffi_type_name() -> &'static str {
        "i32"
    }
}

impl FFI for i64 {
    fn ffi_type_name() -> &'static str {
        "i64"
    }
}

impl FFI for f32 {
    fn ffi_type_name() -> &'static str {
        "f32"
    }
}

impl FFI for f64 {
    fn ffi_type_name() -> &'static str {
        "f64"
    }
}
