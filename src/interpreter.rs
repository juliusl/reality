use crate::Block;

/// Trait to interpret blocks 
/// 
pub trait Interpreter { 
    /// Interpreter output 
    /// 
    type Output;

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

mod test {
    use crate::Interpreter;

    struct Test();

    impl Interpreter for Test {
        type Output = ();

        fn interpret_mut(&mut self, _block: &crate::Block) {
            
        }

        fn interpret(&self, _block: &crate::Block) -> Option<Self::Output> {
            Some(())
        }
    }
}