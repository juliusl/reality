use specs::{Component, VecStorage};

/// Component for a tag identifier,
/// 
#[derive(Component)]
#[storage(VecStorage)]
pub struct Tag(pub String);
