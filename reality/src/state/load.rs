use specs::{Join, Entities, Entity};


/// Trait to implement in order to load state for a specific entity,
/// 
pub trait Load 
where
    Self: Sized
{
    /// The data storage layout,
    /// 
    type Layout: Join;

    /// Loads state for self frin world data,
    /// 
    fn load(entity: Entity, state: <Self::Layout as Join>::Type) -> Self;

    /// Returns the current self from world data if it's data exists,
    /// 
    fn current<'a>(entity: Entity, entities: &Entities<'a>, data: Self::Layout) -> Option<Self> {
        data.join()
            .get(entity, entities)
            .map(|s| Self::load(entity, s))
    }
}

