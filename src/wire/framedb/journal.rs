use specs::{Component, VecStorage};

/// Component that tracks if the stored protocol is ready to read from,
/// 
#[derive(Component, Default)]
#[storage(VecStorage)]
pub struct Journal;