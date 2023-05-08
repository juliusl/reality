use std::collections::BTreeMap;

use crate::Result;
use crate::Identifier;

/// Resource that maps identifiers to a thunk type,
/// 
pub struct ThunkTable<T> {
    /// Table mapping identifiers to thunks that they implement,
    /// 
    table: BTreeMap<Identifier, T>,
}

pub trait Schedule {
    /// Schedules thunks from a thunk table,
    /// 
    fn schedule(&self, table: ThunkTable<()>) -> Result<()>;
}