use std::any::TypeId;
use std::str::FromStr;

use crate::define_intern_table;
use crate::interner::{InternResult, LevelFlags};
use crate::prelude::*;
use crate::interner::InternerFactory;
use crate::push_tag;
use crate::repr::Tag;

// Intern table for owner type ids
define_intern_table!(OWNER_ID: TypeId);

// Intern table for owner names
define_intern_table!(OWNER_NAME: &'static str);

// Intern table for owner type sizes
define_intern_table!(OWNER_SIZE: usize);

// Intern table for field offsets
define_intern_table!(FIELD_OFFSET: usize);

// Intern table for field names
define_intern_table!(FIELD_NAME: &'static str);

/// Trait allowing a type to identify one of it's fields by offset,
///
pub trait Field<const OFFSET: usize>: Send + Sync + 'static {
    /// Associated type that implements FromStr and is the resulting type
    /// when a field has been parsed for this field,
    ///
    type ParseType: FromStr + Send + Sync + 'static;

    /// Associated type that is projected by the implementing type for this field,
    ///
    /// **TODO**: By default this can be the same as the parse type. If associated type defaults
    /// existed, then the default would just be the ParseType.
    ///
    type ProjectedType: Send + Sync + 'static;

    /// Name of the field,
    ///
    fn field_name() -> &'static str;

    /// Creates and returns a representation factory at repr level 1,
    /// 
    fn create_repr<I: InternerFactory + Default>() -> anyhow::Result<ReprFactory<I>>
    where
        Self: Sized,
    {
        let mut factory = ReprFactory::<I>::describe_resource::<Self::ProjectedType>();

        factory.push_level(FieldLevel::new::<OFFSET, Self>())?;

        Ok(factory)
    }
}

/// Field level is the next level of representation,
///
/// Field level asserts the relationship between some owning resource and a field
/// this resource owns.
///
#[derive(Clone, Copy)]
pub struct FieldLevel {
    /// Owner type id,
    ///
    owner_type_id: Tag<TypeId>,
    /// Owner type name,
    ///
    owner_name: Tag<&'static str>,
    /// Owner size,
    ///
    owner_size: Tag<usize>,
    /// Field offset,
    ///
    field_offset: Tag<usize>,
    /// Field name,
    ///
    field_name: Tag<&'static str>,
}

impl FieldLevel {
    /// Creates a new field level representation,
    ///
    pub fn new<const OFFSET: usize, Owner>() -> Self
    where
        Owner: Field<OFFSET> + Send + Sync + 'static,
    {
        Self {
            owner_type_id: Tag::new(&OWNER_ID, std::any::TypeId::of::<Owner>),
            owner_name: Tag::new(&OWNER_NAME, std::any::type_name::<Owner>),
            owner_size: Tag::new(&OWNER_SIZE, std::mem::size_of::<Owner>),
            field_offset: Tag::new(&FIELD_OFFSET, || OFFSET),
            field_name: Tag::new(&FIELD_NAME, Owner::field_name),
        }
    }
}

impl Level for FieldLevel {
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
        push_tag!(interner, self.owner_type_id);
        push_tag!(interner, self.owner_name);
        push_tag!(interner, self.owner_size);
        push_tag!(interner, self.field_offset);
        push_tag!(interner, self.field_name);

        interner.set_level_flags(LevelFlags::LEVEL_1);

        interner.interner()
    }

    type Mount = (
        TypeId,
        &'static str,
        usize,
        usize,
        &'static str,
    );

    fn mount(&self) -> Self::Mount {
        (
            self.owner_type_id.value(),
            self.owner_name.value(),
            self.owner_size.value(),
            self.field_offset.value(),
            self.field_name.value(),
        )
    }
}

/// Wrapper struct to access field tags,
///
pub struct FieldRepr(pub(crate) InternHandle);

impl FieldRepr {
    /// Returns the tag value of the field name,
    ///
    #[inline]
    pub async fn name(&self) -> Option<&'static str> {
        self.0.field_name().await
    }

    /// Returns the tag value of the field offset,
    ///
    #[inline]
    pub async fn offset(&self) -> Option<usize> {
        self.0.field_offset().await
    }

    /// Returns the tag value of the owner type name,
    ///  
    #[inline]
    pub async fn owner_name(&self) -> Option<&'static str> {
        self.0.owner_name().await
    }

    /// Returns the tag value of the owner type size,
    ///
    #[inline]
    pub async fn owner_size(&self) -> Option<usize> {
        self.0.owner_size().await
    }

    /// Returns the tag value of the owner's type id,
    ///
    #[inline]
    pub async fn owner_type_id(&self) -> Option<TypeId> {
        self.0.owner_type_id().await
    }

    /// Returns the tag value of the field name,
    ///
    #[inline]
    pub fn try_name(&self) -> Option<&'static str> {
        self.0.try_field_name()
    }

    /// Returns the tag value of the field offset,
    ///
    #[inline]
    pub fn try_offset(&self) -> Option<usize> {
        self.0.try_field_offset()
    }

    /// Returns the tag value of the owner type name,
    ///  
    #[inline]
    pub fn try_owner_name(&self) -> Option<&'static str> {
        self.0.try_owner_name()
    }

    /// Returns the tag value of the owner type size,
    ///
    #[inline]
    pub fn try_owner_size(&self) -> Option<usize> {
        self.0.try_owner_size()
    }

    /// Returns the tag value of the owner's type id,
    ///
    #[inline]
    pub fn try_owner_type_id(&self) -> Option<TypeId> {
        self.0.try_owner_type_id()
    }
}
