use std::any::TypeId;

/// Struct capturing metadata on an attribute type,
///
#[derive(Hash, Debug)]
pub struct Attribute {
    type_name: &'static str,
    type_id: TypeId,
    type_size: usize,
    idx: usize,
}

impl Attribute {
    /// Returns a new attribute,
    /// 
    pub fn new<T: 'static>(idx: usize) -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_size: std::mem::size_of::<T>(),
            type_id: std::any::TypeId::of::<T>(),
            idx,
        }
    }
}
