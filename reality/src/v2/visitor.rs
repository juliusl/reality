use std::sync::Arc;

use crate::Identifier;
use crate::Value;
use super::Properties;
use super::Property;
use super::Root;
use super::Block;

/// Visitor trait for visiting compiled runmd data,
/// 
#[allow(unused_variables)]
pub trait Visitor {
    /// Visits a block,
    /// 
    fn visit_block(&mut self, block: &Block) {}

    /// Visits a root,
    /// 
    fn visit_root(&mut self, root: &Root) {}

    /// Visits a root extension,
    /// 
    fn visit_extension(&mut self, identifier: &Identifier) {}

    /// Visits a single property value,
    /// 
    fn visit_value(&mut self, name: &String, value: &Value) {}

    /// Visits a list of values,
    /// 
    fn visit_list(&mut self, name: &String, values: &Vec<Value>) {}

    /// Visits an empty property,
    /// 
    fn visit_empty(&mut self, name: &String) {}

    /// Visits readonly properties,
    /// 
    fn visit_readonly(&mut self, properties: Arc<Properties>) {}

    /// Visits a properties map,
    /// 
    /// Note: If overriding the default implementation, visit_property will need to be called manually.
    /// 
    fn visit_properties(&mut self, properties: &Properties) {
        for (name, property) in properties.iter_properties() {
            self.visit_property(name, property);
        }
    }

    /// Visits a property,
    /// 
    /// Note: If overriding the default implementation, visit_value, visit_list, visit_readonly, 
    /// and visit_empty and will need to be called manually.
    /// 
    fn visit_property(&mut self, name: &String, property: &Property) {
        match property {
            Property::Single(value) => self.visit_value(name, value),
            Property::List(values) => self.visit_list(name, values),
            Property::Properties(properties) => self.visit_readonly(properties.clone()),
            Property::Empty => self.visit_empty(name),
        }
    }
}

impl Visitor for () {
    fn visit_block(&mut self, block: &Block) {
        println!("block - {}", block.ident());
    }

    fn visit_root(&mut self, root: &Root) {
        println!("root - {:#}", root.ident);
    }

    fn visit_extension(&mut self, identifier: &Identifier) {
        println!("ext - {:#}", identifier);
    }
}