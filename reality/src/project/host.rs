use crate::{AttributeParser, Shared};

pub trait Host {
    fn register_with(&mut self, plugin: fn(&mut AttributeParser<Shared>));
}