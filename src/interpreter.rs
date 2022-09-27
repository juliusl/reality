use specs::{Component, World};

use crate::Block;

/// Trait to interpret blocks into components,
/// 
pub trait Interpreter
{ 
    /// Initializes the specs world,
    /// 
    /// Initialization could be registering component types, inserting resources, etc.
    /// 
    fn initialize(&self, world: &mut World);

    /// Interprets the block and returns an output component,
    /// 
    /// When the component is inserted, if an existing component was replaced, this function is called again
    /// with the previous component.
    /// 
    fn interpret(&self, world: &World, block: &Block);
}
