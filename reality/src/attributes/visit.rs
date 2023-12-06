use crate::FieldRef;
use crate::PacketRoutes;
use crate::OnParseField;
use crate::Plugin;

/// Field access,
///
#[derive(Debug)]
pub struct Field<'a, T> {
    /// Field owner type name,
    ///
    pub owner: &'static str,
    /// Name of the field,
    ///
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    ///
    pub value: &'a T,
}

/// Mutable field access,
///
#[derive(Debug)]
pub struct FieldMut<'a, T> {
    /// Field owner type name,
    ///
    pub owner: &'static str,
    /// Name of the field,
    ///
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Mutable access to the field,
    ///
    pub value: &'a mut T,
}

/// Field /w owned value,
///
#[derive(Debug)]
pub struct FieldOwned<T> {
    /// Field owner type name,
    ///
    pub owner: String,
    /// Name of the field,
    ///
    pub name: String,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    ///
    pub value: T,
}

/// Trait for visiting fields w/ read-only access,
///
pub trait Visit<T> {
    /// Returns a vector of fields,
    ///
    fn visit(&self) -> Vec<Field<'_, T>>;
}

/// Trait for visiting fields w/ mutable access,
///
pub trait VisitMut<T> {
    /// Returns a vector of fields w/ mutable access,
    ///
    fn visit_mut<'a: 'b, 'b>(&'a mut self) -> Vec<FieldMut<'b, T>>;
}

/// Trait for visiting fields references on the virtual reference,
///
pub trait VisitVirtual<T, Projected>
where
    Self: Plugin + 'static,
    T: 'static,
    Projected: 'static,
{
    /// Returns a vector of field references from the virtual plugin,
    ///
    fn visit_fields<'a>(virt: &'a PacketRoutes<Self>) -> Vec<&'a FieldRef<Self, T, Projected>>;
}

/// Trait for visiting fields references on the virtual reference,
///
pub trait VisitVirtualMut<T, Projected>
where
    Self: Plugin + 'static,
    T: 'static,
    Projected: 'static,
{
    /// Returns a vector of mutable field references from the virtual plugin,
    ///
    fn visit_fields_mut(
        routes: &mut Self::Virtual,
        visit: impl FnMut(&mut FieldRef<Self, T, Projected>),
    );
}

/// Trait for setting a field,
///
pub trait SetField<T> {
    /// Sets a field on the receiver,
    ///
    /// Returns true if successful.
    ///
    fn set_field(&mut self, field: FieldOwned<T>) -> bool;
}
/// Trait for returning field references by offset,
///
pub trait OnReadField<const OFFSET: usize>
where
    Self: Plugin + OnParseField<OFFSET, <Self as OnReadField<OFFSET>>::FieldType>,
{
    /// The field type being read,
    ///
    type FieldType: Send + Sync + 'static;

    /// Reads a field reference from this type,
    ///
    fn read(virt: &Self::Virtual) -> &FieldRef<Self, Self::FieldType, Self::ProjectedType>;
}

/// Trait for returning mutable field references by offset,
///
pub trait OnWriteField<const OFFSET: usize>
where
    Self: Plugin
        + OnReadField<OFFSET>
        + OnParseField<OFFSET, <Self as OnReadField<OFFSET>>::FieldType>,
{
    /// Writes to a field reference from this type,
    ///
    fn write(virt: &mut Self::Virtual)
        -> &mut FieldRef<Self, Self::FieldType, Self::ProjectedType>;
}

#[allow(unused_imports)]
mod tests {
    use std::{
        ops::Index,
        sync::{Arc, OnceLock},
        time::Duration,
    };

    use super::FieldMut;
    use crate::{prelude::*, FieldKey, FrameListener, PacketRoutes};

    pub mod reality {
        pub use crate::*;
        pub mod prelude {
            pub use crate::prelude::*;
        }
    }

    use anyhow::anyhow;
    use async_stream::stream;
    use async_trait::async_trait;
    use futures_util::{pin_mut, StreamExt};
    use serde::Serialize;
    use tokio::{join, time::Instant};

    #[derive(Reality, Clone, Serialize, Default)]
    #[reality(call=test_noop, plugin)]
    struct Test {
        #[reality(derive_fromstr)]
        name: String,
        other: String,
    }

    async fn test_noop(_tc: &mut ThunkContext) -> anyhow::Result<()> {
        Ok(())
    }

    #[test]
    fn test_visit() {
        let mut test = Test {
            name: String::from(""),
            other: String::new(),
        };
        {
            let mut fields = test.visit_mut();
            let mut fields = fields.drain(..);
            if let Some(FieldMut { name, value, .. }) = fields.next() {
                assert_eq!("name", name);
                *value = String::from("hello-world");
            }

            if let Some(FieldMut { name, value, .. }) = fields.next() {
                assert_eq!("other", name);
                *value = String::from("hello-world-2");
            }
        }

        assert_eq!("hello-world", test.name.as_str());
        assert_eq!("hello-world-2", test.other.as_str());
    }

}
