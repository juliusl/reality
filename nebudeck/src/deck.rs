use std::io::{prelude::*, LineWriter};

use loopio::LoopType;

/// Struct containing a series of devices,
/// 
pub struct Deck;

/// Trait for implementing a Device that can be installed onto a Deck,
/// 
pub trait Device : LoopType + Write + Read + Sized {
    /// Returns input for self,
    /// 
    fn input(self) -> std::io::LineWriter<Self> {
        std::io::LineWriter::new(self)
    }

    fn output();
}

impl Write for Deck {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        // todo!()
        Ok(0)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Read for Deck {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

impl LoopType for Deck {
    fn init() -> Self {
        Deck
    }

    fn body_mut(&mut self) {
        
    }
}