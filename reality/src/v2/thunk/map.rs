use specs::Component;
use crate::Error;

/// Trait for map closure,
///
pub trait Map<C>
where
    Self: Component,
    <Self as Component>::Storage: Default,
    C: Component,
    <C as Component>::Storage: Default
{
    /// Maps a component from self,
    ///
    fn map(&self) -> Result<C, Error>;
}

/// Trait for a map_with closure,
///
pub trait MapWith<C: Component>
where
    Self: Component,
    <Self as Component>::Storage: Default,
    <C as Component>::Storage: Default
{
    /// Maps a component from self with another Component,
    ///
    fn map_with(&self, with: &C) -> Result<Self, Error>;
}
