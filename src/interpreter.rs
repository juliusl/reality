use specs::{Component, World};

use crate::Block;

/// Trait to interpret blocks 
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
    fn interpret(&self, block: &Block) -> Option<Self::Output>;

    /// Interprets a block and updates self,
    /// 
    fn interpret_mut(&mut self, block: &Block);
}
