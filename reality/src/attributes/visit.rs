use crate::FieldRef;
use crate::OnParseField;
use crate::PacketRoutes;
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
    fn visit_fields(virt: &PacketRoutes<Self>) -> Vec<&FieldRef<Self, T, Projected>>;
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
    Self: Plugin + OnParseField<OFFSET>,
{
    /// Reads a field reference from this type,
    ///
    fn read(virt: &Self::Virtual) -> &FieldRef<Self, Self::ParseType, Self::ProjectedType>;
}

/// Trait for returning mutable field references by offset,
///
pub trait OnWriteField<const OFFSET: usize>
where
    Self: Plugin + OnReadField<OFFSET> + OnParseField<OFFSET>,
{
    /// Writes to a field reference from this type,
    ///
    fn write(virt: &mut Self::Virtual)
        -> &mut FieldRef<Self, Self::ParseType, Self::ProjectedType>;
}
