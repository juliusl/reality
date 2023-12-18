use std::any::TypeId;
use std::ops::Deref;
use std::sync::Arc;

use anyhow::anyhow;

use crate::interner::InternResult;
use crate::interner::LevelFlags;
use crate::prelude::*;

use crate::define_intern_table;

// Intern table for resource type names
define_intern_table!(TYPE_NAME: &'static str);

// Intern table for resource type sizes
define_intern_table!(TYPE_SIZE: usize);

// Intern table for resource type ids
define_intern_table!(TYPE_ID: TypeId);

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

// Intern table for input values
define_intern_table!(INPUT: String);

// Intern table for tag values
define_intern_table!(TAG: String);

// Intern table for address values
define_intern_table!(ADDRESS: String);

// Intern table for intern handles
define_intern_table!(HANDLES: InternHandle);

/// Trait for each level of representation that defines how
/// each level configures the intern handle representing a resource.
///
pub trait Level {
    /// Configures the representation state,
    ///
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult;
}

/// Each level of runtime representation is defined by a set of tags,
///
#[derive(Clone, Copy)]
pub(crate) struct Tag<T: Send + Sync + 'static, F: Sync = fn() -> T> {
    /// Table that contains the tag value,
    ///
    pub(crate) intern_table: &'static InternTable<T>,
    /// Create value method,
    ///
    pub(crate) create_value: F,
}

impl<T: Send + Sync + 'static, F: Sync> Tag<T, F> {
    /// Returns a new tag,
    ///
    pub const fn new(intern_table: &'static InternTable<T>, create_value: F) -> Self {
        Self {
            intern_table,
            create_value,
        }
    }
}

impl<T: Send + Sync + 'static> Tag<T> {
    /// Assigns a value to an intern handle,
    ///
    pub async fn assign(&self, handle: InternHandle) -> anyhow::Result<()> {
        self.intern_table
            .assign_intern(handle, (self.create_value)())
            .await
    }

    /// Returns the inner value,
    ///
    pub fn value(&self) -> T {
        (self.create_value)()
    }
}

impl<T: ToOwned<Owned = T> + Send + Sync + 'static> Tag<T, Arc<T>> {
    /// Assign a value to an intern handle,
    ///
    pub async fn assign(&self, handle: InternHandle) -> anyhow::Result<()> {
        self.intern_table
            .assign_intern(handle, self.create_value.deref().to_owned())
            .await
    }

    /// Returns the inner value,
    ///
    pub fn value(&self) -> T {
        self.create_value.deref().to_owned()
    }
}

/// Pushes a tag and a future that can assign an intern handle for a value,
///
macro_rules! push_tag {
    ($interner:ident, $tag:expr) => {
        let tag = $tag;
        $interner.push_tag(tag.value(), move |h| {
            Box::pin(async move { tag.assign(h).await })
        });
    };
    (dyn $interner:ident, $tag:expr) => {
        let tag = $tag;

        let inner = tag.clone();
        $interner.push_tag(tag.value(), move |h| {
            Box::pin(async move { inner.assign(h).await })
        });
    };
}

/// Resource level is the lowest level of representation,
///
/// Resource level asserts compiler information for the resource.
///
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

        interner.interner()
    }
}

/// Field level is the next level of representation,
///
/// Field level asserts the relationship between some owning resource and a field
/// this resource owns.
///
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
}

/// Input level is a dynamic level of representation,
///
/// Input level asserts and records the input paramters for some resource.
///
pub struct InputLevel {
    /// Runmd expression representing this resource,
    ///
    input: Tag<String, Arc<String>>,
    /// Tag value assigned to this resource,
    ///
    tag: Tag<String, Arc<String>>,
}

impl InputLevel {
    /// Creates a new input level representation,
    ///
    pub fn new(input: impl Into<String>, tag: impl Into<String>) -> Self {
        Self {
            input: Tag::new(&INPUT, Arc::new(input.into())),
            tag: Tag::new(&TAG, Arc::new(tag.into())),
        }
    }
}

impl Level for InputLevel {
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
        push_tag!(dyn interner, &self.input);
        push_tag!(dyn interner, &self.tag);

        interner.set_level_flags(LevelFlags::LEVEL_2);

        interner.interner()
    }
}

/// Host level is the upper most level of representation,
///
/// Host level assigns addresses defined by the document structure to the
/// actual resource.
///
pub struct HostLevel {
    /// The address is derived by the documentation hierarchy from runmd and
    /// is some human-readable string associated to some resource.
    ///
    address: Tag<String, Arc<String>>,
}

impl HostLevel {
    /// Creates a new host level representation,
    ///
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: Tag::new(&ADDRESS, Arc::new(address.into())),
        }
    }
}

impl Level for HostLevel {
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
        push_tag!(dyn interner, &self.address);

        interner.set_level_flags(LevelFlags::LEVEL_3);

        interner.interner()
    }
}

/// Factory for constructing a repr,
///
#[derive(Default)]
pub struct ReprFactory<I = CrcInterner>
where
    I: InternerFactory,
{
    /// Interner,
    ///
    interner: I,
    /// Vector of intern handles tags for each level of the current representation,
    ///
    levels: Vec<Tag<InternHandle, Arc<InternHandle>>>,
}

impl<I: InternerFactory + Default> ReprFactory<I> {
    /// Creates a new repr w/ the root as the ResourceLevel,
    ///
    pub fn describe_resource<T: Send + Sync + 'static>() -> Self {
        let mut repr = ReprFactory::default();

        repr.push_level(ResourceLevel::new::<T>())
            .expect("should be able to push since the repr is empty");

        repr
    }

    /// Pushes a level to the current stack of levels,
    ///
    pub fn push_level(&mut self, level: impl Level) -> anyhow::Result<()> {
        let handle = level.configure(&mut self.interner).result()?;

        // Handle errors
        if let Some(last) = self.levels.last() {
            let flag = last.create_value.level_flags();

            if flag != LevelFlags::from_bits_truncate(handle.level_flags().bits() >> 1) {
                Err(anyhow!("Expected next level"))?;
            }
        } else if handle.level_flags().bits() != 0 {
            Err(anyhow!("Expected root level"))?;
        }

        self.levels.push(Tag::new(&HANDLES, Arc::new(handle)));

        Ok(())
    }

    /// Returns the current representation level,
    ///
    pub fn level(&self) -> usize {
        self.levels.len() - 1
    }
}
