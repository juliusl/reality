use specs::{Join, Entities, Entity};


/// Trait to implement in order to load state for a specific entity,
/// 
pub trait Loader 
where
    Self: Sized
{
    /// The type of world data required to load self,
    /// 
    type Data: Join;

    /// Loads state for self frin world data,
    /// 
    fn load(state: <Self::Data as Join>::Type) -> Self;

    /// Returns the current self from world data if it's data exists,
    /// 
    fn current<'a>(entity: Entity, entities: &Entities<'a>, data: Self::Data) -> Option<Self> {
        data.join()
            .get(entity, entities)
            .map(|s| Self::load(s))
    }
}

