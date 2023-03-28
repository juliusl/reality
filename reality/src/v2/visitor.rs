use std::collections::BTreeSet;
use std::sync::Arc;

use crate::Identifier;
use crate::Value;
use super::Object;
use super::Properties;
use super::Property;
use super::Root;
use super::Block;

/// Visitor trait for visiting compiled runmd data,
/// 
/// Note: Includes a number of default implementations mostly for the non-leaf types,
/// 
#[allow(unused_variables)]
pub trait Visitor {
    /// Visits an empty value,
    /// 
    fn visit_empty_value(&mut self, name: &String, idx: Option<usize>) {}

    /// Visits a bool value,
    /// 
    fn visit_bool(&mut self, name: &String, idx: Option<usize>, bool: bool) {}

    /// Visits a symbol value,
    /// 
    fn visit_symbol(&mut self, name: &String, idx: Option<usize>, symbol: &String) {}

    /// Visits a text buffer value,
    /// 
    fn visit_text_buffer(&mut self, name: &String, idx: Option<usize>, text_buffer: &String) {}

    /// Visits an integer value,
    /// 
    fn visit_int(&mut self, name: &String, idx: Option<usize>, i: i32) {}

    /// Visits an integer pair value,
    /// 
    fn visit_int_pair(&mut self, name: &String, idx: Option<usize>, pair: &[i32; 2]) {}

    /// Visits an integer range value,
    /// 
    fn visit_int_range(&mut self, name: &String, idx: Option<usize>, range: &[i32; 3]) {}

    /// Visits a float value,
    /// 
    fn visit_float(&mut self, name: &String, idx: Option<usize>, f: f32) {}

    /// Visits a float pair value,
    /// 
    fn visit_float_pair(&mut self, name: &String, idx: Option<usize>, pair: &[f32; 2]) {}

    /// Visits a float range value,
    /// 
    fn visit_float_range(&mut self, name: &String, idx: Option<usize>, range: &[f32; 3]) {}

    /// Visits a binary value,
    /// 
    fn visit_binary(&mut self, name: &String, idx: Option<usize>, binary: &Vec<u8>) {}

    /// Visits a reference value,
    /// 
    fn visit_reference(&mut self, name: &String, idx: Option<usize>, reference: u64) {}

    /// Visits a complex set value,
    /// 
    fn visit_complex(&mut self, name: &String, idx: Option<usize>, complex: &BTreeSet<String>) {}

    /// Visits a root extension,
    /// 
    /// Note: By default, will be called in parse order
    /// 
    fn visit_extension(&mut self, identifier: &Identifier) {}

    /// Visits an identifier,
    /// 
    fn visit_identifier(&mut self, identifier: &Identifier) {}

    /// Visits an empty property,
    /// 
    fn visit_empty(&mut self, name: &String) {}

    /// Visits readonly properties,
    /// 
    fn visit_readonly(&mut self, properties: Arc<Properties>) {}

    /// Visits a property name,
    /// 
    fn visit_property_name(&mut self, name: &String) {}

    /// Visits a list of values,
    /// 
    /// Default implementation will call this method on the alternate if some,
    /// 
    /// Note: if overriding default implementation, value idx will need to be derived if calling visit_value
    /// 
    fn visit_list(&mut self, name: &String, values: &Vec<Value>) {
        for (idx, v) in values.iter().enumerate() {
            self.visit_value(name, Some(idx), v);
        }
    }

    /// Visits a property value,
    /// 
    /// Default implementation will call this method on the alternate if some,
    /// 
    /// Note: If overriding default implementation, visit_* value types will need to be called manually
    /// 
    fn visit_value(&mut self, name: &String, idx: Option<usize>, value: &Value) {
        match value {
            Value::Empty => self.visit_empty_value(name, idx),
            Value::Bool(b) => self.visit_bool(name, idx, *b),
            Value::TextBuffer(t) => self.visit_text_buffer(name, idx, t),
            Value::Int(i) => self.visit_int(name, idx, *i),
            Value::IntPair(i1, i2) => self.visit_int_pair(name, idx, &[*i1, *i2]),
            Value::IntRange(i1, i2, i3) => self.visit_int_range(name, idx, &[*i1, *i2, *i3]),
            Value::Float(f) => self.visit_float(name, idx, *f),
            Value::FloatPair(f1, f2) => self.visit_float_pair(name, idx, &[*f1, *f2]),
            Value::FloatRange(f1, f2, f3) => self.visit_float_range(name, idx, &[*f1, *f2, *f3]),
            Value::BinaryVector(b) => self.visit_binary(name, idx, b),
            Value::Symbol(s) => self.visit_symbol(name, idx, s),
            Value::Reference(r) => self.visit_reference(name, idx, *r),
            Value::Complex(c) => self.visit_complex(name, idx, c),
        }
    }

    /// Visits an object,
    /// 
    /// Default implementation will call this method on the alternate if some,
    /// 
    /// Note: If overriding the default implementation, visit_block, visit_identifier, and visit_properties, will need to be called manually.
    /// 
    fn visit_object(&mut self, object: &Object) {
        object.as_block().map(|b| self.visit_block(b));
        object.as_root().map(|b| self.visit_root(b));
        self.visit_identifier(object.ident());
        self.visit_properties(object.properties());
    }

    /// Visits a block,
    /// 
    /// Default implementation will call this method on the alternate if some,
    /// 
    /// Note: If overriding the default implementation, visit_root will need to be called manually.
    /// 
    fn visit_block(&mut self, block: &Block) {
        for root in block.roots() {
            self.visit_root(root);
        }
    }

    /// Visits a root,
    /// 
    /// Default implementation will call this method on the alternate if some,
    /// 
    /// Note: If overriding the default implementation, visit_extension will need to be called manually.
    /// 
    fn visit_root(&mut self, root: &Root) {
        for ext in root.extensions() {
            self.visit_extension(ext);
        }
    }

    /// Visits a properties map,
    /// 
    /// Default implementation will call this method on the alternate if some,
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
    /// Default implementation will call this method on the alternate if some,
    /// 
    /// Note: If overriding the default implementation, visit_value, visit_list, visit_readonly, 
    /// and visit_empty and will need to be called manually.
    /// 
    fn visit_property(&mut self, name: &String, property: &Property) {
        self.visit_property_name(name);

        match property {
            Property::Single(value) => self.visit_value(name, None, value),
            Property::List(values) => self.visit_list(name, values),
            Property::Properties(properties) => self.visit_readonly(properties.clone()),
            Property::Empty => self.visit_empty(name),
        }
    }
}

impl Visitor for () {
    fn visit_identifier(&mut self, identifier: &Identifier) {
        println!("{:#?}", identifier);
    }
}