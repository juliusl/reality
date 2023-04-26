use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

use bytemuck::cast;
use bytes::BufMut;
use bytes::BytesMut;
use tracing::trace;

use super::Block;
use super::Properties;
use super::Property;
use super::Root;
use crate::v2::states::Object;
use crate::Identifier;
use crate::Value;

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

    /// Visits a root extension,
    ///
    fn visit_extension(&mut self, identifier: &Identifier) {}

    /// Visits a list of values,
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

    /// Visits a properties map,
    ///
    /// Note: If overriding the default implementation, visit_property will need to be called manually.
    ///
    fn visit_properties(&mut self, properties: &Properties) {
        self.visit_identifier(properties.owner());

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
        self.visit_property_name(name);

        match property {
            Property::Single(value) => self.visit_value(name, None, value),
            Property::List(values) => self.visit_list(name, values),
            Property::Properties(properties) => self.visit_readonly(properties.clone()),
            Property::Empty => self.visit_empty(name),
        }
    }

    /// Visits an object,
    ///
    /// # Background
    ///
    /// An object struct represents transient data with 3 variants, Block, Root, and Extension.
    /// Because they are transient, they include an entity in their fn inputs.
    ///
    /// Note: If overriding the default implementation, visit_block, visit_identifier, and visit_properties, will need to be called manually.
    ///
    fn visit_object(&mut self, object: &Object) {
        // Transient data
        object
            .as_block()
            .map(|b| self.visit_block(b));

        object
            .as_root()
            .map(|b| self.visit_root(b));

        // Stable data
        self.visit_identifier(object.ident());
        self.visit_properties(object.properties());
    }

    /// Visits a block,
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
    /// Note: If overriding the default implementation, visit_extension will need to be called manually.
    ///
    fn visit_root(&mut self, root: &Root) {
        for ext in root.extensions() {
            self.visit_extension(ext);
        }
    }
}

impl Visitor for () {
    fn visit_identifier(&mut self, identifier: &Identifier) {
        println!("{:#?}", identifier);
    }

    fn visit_property(&mut self, name: &String, property: &Property) {
        trace!("name: {name}, property: {:?}", property);
    }
}

impl Visitor for String {
    fn visit_symbol(&mut self, _: &String, _: Option<usize>, symbol: &String) {
        *self = symbol.to_string();
    }

    fn visit_text_buffer(&mut self, _: &String, _: Option<usize>, text_buffer: &String) {
        *self = text_buffer.to_string();
    }
}

impl Visitor for Vec<String> {
    fn visit_symbol(&mut self, _: &String, idx: Option<usize>, symbol: &String) {
        if let Some(idx) = idx {
            if self.get(idx).is_some() {
            } else {
                self.insert(idx, symbol.to_string());
            }
        } else {
            self.push(symbol.to_string());
        }
    }

    fn visit_text_buffer(&mut self, _: &String, idx: Option<usize>, text_buffer: &String) {
        if let Some(idx) = idx {
            self.insert(idx, text_buffer.to_string());
        } else {
            self.push(text_buffer.to_string());
        }
    }
}

impl Visitor for bool {
    fn visit_bool(&mut self, _: &String, _: Option<usize>, bool: bool) {
        *self = bool;
    }
}

impl Visitor for usize {
    fn visit_int(&mut self, _: &String, _: Option<usize>, i: i32) {
        if i >= 0 {
            *self = i as usize;
        } else {
            // Skipping because integer is signed
        }
    }
}

impl Visitor for i32 {
    fn visit_int(&mut self, _: &String, _: Option<usize>, i: i32) {
        *self = i;
    }
}

impl Visitor for u32 {
    fn visit_int(&mut self, _: &String, _: Option<usize>, i: i32) {
        if i >= 0 {
            *self = i as u32;
        } else {
            // Skipping because integer is signed
        }
    }
}

impl Visitor for crate::Result<usize> {
    fn visit_int(&mut self, _: &String, _: Option<usize>, i: i32) {
        if i >= 0 {
            *self = Ok(i as usize);
        } else {
            *self = Err("Current value is signed (negative) and cannot be converted".into())
        }
    }

    fn visit_int_pair(&mut self, _: &String, _: Option<usize>, pair: &[i32; 2]) {
        if pair[0] >= 0 && pair[1] >= 0 {
            *self = Ok(cast::<[i32; 2], usize>(*pair));
        } else {
            *self = Err("Current value is signed (negative) and cannot be converted".into())
        }
    }
}

impl Visitor for crate::Result<u32> {
    fn visit_int(&mut self, _: &String, _: Option<usize>, i: i32) {
        if i >= 0 {
            *self = Ok(i as u32);
        } else {
            *self = Err("Current value is signed (negative) and cannot be converted".into())
        }
    }
}

impl Visitor for crate::Result<i64> {
    fn visit_int(&mut self, _: &String, _: Option<usize>, i: i32) {
        *self = Ok(i as i64);
    }

    fn visit_int_pair(&mut self, _: &String, _: Option<usize>, pair: &[i32; 2]) {
        *self = Ok(cast::<[i32; 2], i64>(*pair))
    }
}

impl Visitor for Vec<u8> {
    fn visit_binary(&mut self, _: &String, _: Option<usize>, binary: &Vec<u8>) {
        if self.len() == binary.len() {
            self.copy_from_slice(&binary);
        } else {
            // Skipping because vectors are different lengths
        }
    }

    fn visit_text_buffer(&mut self, _: &String, _: Option<usize>, text_buffer: &String) {
        if self.len() == text_buffer.as_bytes().len() {
            self.copy_from_slice(text_buffer.as_bytes());
        } else {
            // Skipping because vectors are different lengths
        }
    }
}

impl Visitor for BytesMut {
    fn visit_binary(&mut self, _: &String, _: Option<usize>, binary: &Vec<u8>) {
        self.put(&binary[..]);
    }

    fn visit_text_buffer(&mut self, _: &String, _: Option<usize>, text_buffer: &String) {
        self.put(text_buffer.as_bytes());
    }

    fn visit_readonly(&mut self, properties: Arc<Properties>) {
        let name = properties.owner().subject();

        let property = Property::Properties(properties);
        if let Some(bytes) = property.as_binary() {
            self.visit_binary(&name, None, bytes);
        } else if let Some(text_buffer) = property.as_text() {
            self.visit_text_buffer(&name, None, text_buffer);
        }
    }
}

impl Visitor for BTreeMap<String, Value> {
    fn visit_value(&mut self, name: &String, idx: Option<usize>, value: &Value) {
        if idx.is_none() {
            self.insert(name.to_string(), value.clone());
        }
    }
}

/// # Experiment
/// 
/// How to use visitor pattern directly with world storage.
/// 
mod experiment_specs {
    use crate::v2::{compiler::Object, prelude::*};

    impl<'a> Visitor for WriteStorage<'a, Properties> {
        fn visit_object(&mut self, object: &Object) {
            self.insert(object.entity(), object.properties().clone())
                .ok();
        }
    }

    impl<'a> Visitor for WriteStorage<'a, Identifier> {
        fn visit_object(&mut self, object: &Object) {
            // This will only work if the target storage belongs to a world where these entities are alive
            self.insert(object.entity(), object.ident().clone()).ok();
        }
    }

    #[allow(unused_imports)]
    mod tests {
        use crate::v2::prelude::*;

        #[test]
        fn test_transfer() -> Result<()> {
            let mut compiler = Compiler::new();
            let _ = Parser::new()
                .parse_line("```runmd")?
                .parse_line("+ .test A")?
                .parse_line("<> .comp ")?
                .parse("```", &mut compiler)?;
            
            let build = compiler.compile()?;
            compiler
                .as_mut()
                .exec(|(entities, idents): (Entities, ReadStorage<Identifier>)| {
                    for (entity, ident) in (&entities, &idents).join() {
                        println!("Original -- {:?} :: {:#}", entity, ident);
                    }
                });

            let mut transfer = World::new();
            transfer.register::<Identifier>();
            transfer.entities().create();
            transfer.entities().create();
            transfer.entities().create();
            transfer.entities().create();
            transfer.entities().create();    
            transfer.maintain();

            compiler
                .compiled()
                .visit_build(build, &mut transfer.write_component::<Identifier>());
            transfer.maintain();

            transfer.exec(|(entities, idents): (Entities, ReadStorage<Identifier>)| {
                for (entity, ident) in (&entities, &idents).join() {
                    println!("Transfer -- {:?} :: {:#}", entity, ident);
                }
            });

            Ok(())
        }
    }
}
