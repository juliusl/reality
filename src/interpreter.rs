use specs::{Component, World};

use crate::Block;

/// Trait to interpret blocks into components,
/// 
pub trait Interpreter
{ 
    /// Interpreter output 
    /// 
    type Output: Component;

    /// Initializes the specs world,
    /// 
    /// Initialization could be registering component types, inserting resources, etc.
    /// 
    fn initialize(&self, world: &mut World);

    /// Returns a future after interpreting block, 
    /// 
    /// If returns None, means that the block does not require 
    /// any further interpretation
    /// 
    fn interpret(&self, block: &Block, previous: Option<&Self::Output>) -> Option<Self::Output>;
}
