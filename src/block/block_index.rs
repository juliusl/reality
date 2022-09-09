use std::collections::BTreeMap;

use atlier::system::Value;

/// This struct takes a property map, and from each `.complex` value,
/// indexes a subset of the map.
/// 
pub struct BlockIndex; 

impl From<BTreeMap<String, Value>> for BlockIndex {
    fn from(_: BTreeMap<String, Value>) -> Self {
        todo!()
    }
}