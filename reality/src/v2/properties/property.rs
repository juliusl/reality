use crate::Value;
use crate::v2::Visitor;
use std::fmt::Display;
use std::ops::Index;
use std::sync::Arc;

use super::Properties;

/// Enumeration of property types
///
#[derive(Default, Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Property {
    /// Property is a single value,
    ///
    Single(Value),
    /// Property is a list of values,
    ///
    List(Vec<Value>),
    /// Property is a read-only reference to block properties,
    ///
    Properties(Arc<Properties>),
    /// Indicates that this block property is currently empty,
    ///
    #[default]
    Empty,
}

impl Property {
    /// Returns true if the property is a bool value and true
    ///
    pub fn is_enabled(&self) -> bool {
        match self {
            Property::Single(Value::Bool(enabled)) => *enabled,
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].is_enabled()
            }
            _ => false,
        }
    }

    /// Returns a string if the property is a single text buffer
    ///
    pub fn as_text(&self) -> Option<&String> {
        match self {
            Property::Single(Value::TextBuffer(text)) => Some(text),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_text()
            }
            _ => None,
        }
    }

    /// Returns a reference to a String if the property is a single symbol
    ///
    pub fn as_symbol(&self) -> Option<&String> {
        match self {
            Property::Single(Value::Symbol(symbol)) => Some(symbol),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_symbol()
            }
            _ => None,
        }
    }

    /// Returns a &str if the property is a single symbol,
    ///
    pub fn as_symbol_str(&self) -> Option<&str> {
        self.as_symbol().map(|s| s.as_str())
    }

    /// Returns an integer if the property is an int,
    ///
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Property::Single(Value::Int(i)) => Some(*i),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_int()
            }
            _ => None,
        }
    }

    /// Returns a float if the property is a float,
    ///
    pub fn as_float(&self) -> Option<f32> {
        match self {
            Property::Single(Value::Float(f)) => Some(*f),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_float()
            }
            _ => None,
        }
    }

    /// Returns an array of two floats if property is a float pair,
    /// 
    pub fn as_float2(&self) -> Option<[f32; 2]> {
        match self {
            Property::Single(Value::FloatPair(f1, f2)) => Some([*f1, *f2]),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_float2()
            }
            _ => None,
        }
    }

    /// Returns an array 
    /// 
    pub fn as_int2(&self) -> Option<[i32; 2]> {
        match self {
            Property::Single(Value::IntPair(i1, i2)) => Some([*i1, *i2]),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_int2()
            }
            _ => None,
        }
    }

    /// Returns an array of three floats if property is a float range,
    /// 
    pub fn as_float3(&self) -> Option<[f32; 3]> {
        match self {
            Property::Single(Value::FloatRange(f1, f2, f3)) => Some([*f1, *f2, *f3]),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_float3()
            }
            _ => None,
        }
    }

    /// Returns an array of three integers if property is a int range,
    /// 
    pub fn as_int3(&self) -> Option<[i32; 3]> {
        match self {
            Property::Single(Value::IntRange(i1, i2, i3)) => Some([*i1, *i2, *i3]),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_int3()
            }
            _ => None,
        }
    }

    /// Returns a vector of strings if the property is a single text buffer,
    /// or if the property is a list of values, filters all text buffers
    ///
    pub fn as_text_vec(&self) -> Option<Vec<String>> {
        match self {
            Property::Single(Value::TextBuffer(text)) => Some(vec![text.to_string()]),
            Property::List(values) => {
                Some(values.iter().filter_map(Value::text).collect::<Vec<_>>())
            }
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_text_vec()
            }
            _ => None,
        }
    }

    /// Returns a vector of strings if the property is a single symbol,
    /// or if the property is a list of values, filters all symbols
    ///
    pub fn as_symbol_vec(&self) -> Option<Vec<String>> {
        match self {
            Property::Single(Value::Symbol(text)) => Some(vec![text.to_string()]),
            Property::List(values) => {
                Some(values.iter().filter_map(Value::symbol).collect::<Vec<_>>())
            }
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_symbol_vec()
            }
            _ => None,
        }
    }

    /// Returns a vector of integers if the property is a single int,
    /// or if the property is a list of values, filters all ints
    ///
    pub fn as_int_vec(&self) -> Option<Vec<i32>> {
        match self {
            Property::Single(Value::Int(int)) => Some(vec![*int]),
            Property::List(values) => {
                Some(values.iter().filter_map(Value::int).collect::<Vec<_>>())
            }
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_int_vec()
            }
            _ => None,
        }
    }

    /// Returns a vector of integers if the property is a single int,
    /// or if the property is a list of values, filters all ints
    ///
    pub fn as_float_vec(&self) -> Option<Vec<f32>> {
        match self {
            Property::Single(Value::Float(float)) => Some(vec![*float]),
            Property::List(values) => {
                Some(values.iter().filter_map(Value::float).collect::<Vec<_>>())
            }
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_float_vec()
            }
            _ => None,
        }
    }

    /// Returns a vector of values,
    ///
    pub fn as_value_vec(&self) -> Option<&Vec<Value>> {
        match self {
            Property::List(list) => Some(list),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_value_vec()
            }
            _ => None,
        }
    }

    /// Returns a binary vector,
    ///
    pub fn as_binary(&self) -> Option<&Vec<u8>> {
        match self {
            Property::Single(Value::BinaryVector(bin)) => Some(bin),
            Property::Properties(properties) => {
                let key = properties.owner().subject();
                properties[&key].as_binary()
            }
            _ => None,
        }
    }

    /// Returns as properties,
    ///
    pub fn as_properties(&self) -> Option<Arc<Properties>> {
        match self {
            Property::Properties(properties) => Some(properties.clone()),
            _ => None,
        }
    }

    /// Edits the value of this property,
    ///
    pub fn edit(
        &mut self,
        on_single: impl Fn(&mut Value),
        on_list: impl Fn(&mut Vec<Value>),
        on_empty: impl Fn() -> Option<Value>,
    ) {
        match self {
            Property::Single(single) => on_single(single),
            Property::List(list) => on_list(list.as_mut()),
            Property::Properties(_) => {
                // read-only
                return;
            }
            Property::Empty => match on_empty() {
                Some(value) => *self = Property::Single(value),
                None => {}
            },
        }
    }
}

impl Display for Property {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Property::Single(single) => display_value(f, single),
            Property::List(list) => {
                for val in list {
                    display_value(f, val)?;
                    write!(f, ", ")?;
                }
                Ok(())
            }
            Property::Properties(_) => write!(f, "properties - todo display"),
            Property::Empty => write!(f, "empty value"),
        }
    }
}

/// Function to display a value,
///
pub fn display_value(f: &mut std::fmt::Formatter<'_>, value: &Value) -> std::fmt::Result {
    match value {
        Value::Empty => write!(f, "()"),
        Value::Bool(b) => write!(f, "{b}"),
        Value::TextBuffer(t) => write!(f, "{t}"),
        Value::Int(i) => write!(f, "{i}"),
        Value::IntPair(i1, i2) => write!(f, "[{i1}, {i2}]"),
        Value::IntRange(i1, i2, i3) => write!(f, "[{i1}, {i2}, {i3}]"),
        Value::Float(f1) => write!(f, "{f1}"),
        Value::FloatPair(f1, f2) => write!(f, "[{f1}, {f2}]"),
        Value::FloatRange(f1, f2, f3) => write!(f, "[{f1}, {f2}, {f3}]"),
        Value::BinaryVector(bin) => write!(f, "binary-vector omitted, len: {}", bin.len()),
        Value::Reference(r) => write!(f, "ref:{r}"),
        Value::Symbol(s) => write!(f, "{s}"),
        Value::Complex(c) => write!(f, "{:?}", c),
    }
}

impl<'a> Index<&'a str> for Property {
    type Output = Property;

    fn index(&self, index: &'a str) -> &Self::Output {
        match self {
            Property::Properties(props) => props.property(index).unwrap_or(&Property::Empty),
            _ => &Property::Empty,
        }
    }
}

impl<'a> Index<usize> for Property {
    type Output = Value;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            Property::List(values) => values.get(index).unwrap_or(&Value::Empty),
            _ => &Value::Empty,
        }
    }
}

impl From<Arc<Properties>> for Property {
    fn from(value: Arc<Properties>) -> Self {
        Property::Properties(value)
    }
}

impl From<Value> for Property {
    fn from(value: Value) -> Self {
        Property::Single(value)
    }
}

impl From<Vec<Value>> for Property {
    fn from(value: Vec<Value>) -> Self {
        Property::List(value)
    }
}

impl From<&Property> for String {
    fn from(value: &Property) -> Self {
        value.as_symbol().map(|s| s.to_string()).unwrap_or_default()
    }
}

impl From<Property> for String {
    fn from(value: Property) -> Self {
        value.as_symbol().map(|s| s.to_string()).unwrap_or_default()
    }
}

impl From<Property> for usize {
    fn from(value: Property) -> Self {
        value.as_int().map(|s| s as usize).unwrap_or_default()
    }
}

impl From<&Property> for usize {
    fn from(value: &Property) -> Self {
        value.as_int().map(|s| s as usize).unwrap_or_default()
    }
}

impl From<Property> for bool {
    fn from(value: Property) -> Self {
        value.is_enabled()
    }
}

impl From<&Property> for bool {
    fn from(value: &Property) -> Self {
        value.is_enabled()
    }
}

impl From<Property> for i32 {
    fn from(value: Property) -> Self {
        value.as_int().unwrap_or_default()
    }
}

impl From<&Property> for i32 {
    fn from(value: &Property) -> Self {
        value.as_int().unwrap_or_default()
    }
}

impl From<Property> for f32 {
    fn from(value: Property) -> Self {
        value.as_float().unwrap_or_default()
    }
}

impl From<&Property> for f32 {
    fn from(value: &Property) -> Self {
        value.as_float().unwrap_or_default()
    }
}

impl From<Property> for [f32; 2] {
    fn from(value: Property) -> Self {
        value.as_float2().unwrap_or_default()
    }
}

impl From<&Property> for [f32; 2] {
    fn from(value: &Property) -> Self {
        value.as_float2().unwrap_or_default()
    }
}

impl From<Property> for [i32; 2] {
    fn from(value: Property) -> Self {
        value.as_int2().unwrap_or_default()
    }
}

impl From<&Property> for [i32; 2] {
    fn from(value: &Property) -> Self {
        value.as_int2().unwrap_or_default()
    }
}

impl From<Property> for [i32; 3] {
    fn from(value: Property) -> Self {
        value.as_int3().unwrap_or_default()
    }
}

impl From<&Property> for [i32; 3] {
    fn from(value: &Property) -> Self {
        value.as_int3().unwrap_or_default()
    }
}

impl From<Property> for [f32; 3] {
    fn from(value: Property) -> Self {
        value.as_float3().unwrap_or_default()
    }
}

impl From<&Property> for [f32; 3] {
    fn from(value: &Property) -> Self {
        value.as_float3().unwrap_or_default()
    }
}

impl From<Property> for Vec<String> {
    fn from(value: Property) -> Self {
        value.as_symbol_vec().unwrap_or_default()
    }
}

impl From<&Property> for Vec<String> {
    fn from(value: &Property) -> Self {
        value.as_symbol_vec().unwrap_or_default()
    }
}

impl From<Property> for () {
    fn from(_: Property) -> Self {
        ()
    }
}

impl From<&Property> for () {
    fn from(_: &Property) -> Self {
        ()
    }
}

/// Returns a property from a value,
///
pub fn property_value(value: impl Into<Value>) -> Property {
    Property::Single(value.into())
}

/// Returns a property list from an iterator,
///
pub fn property_list(list: impl IntoIterator<Item = impl Into<Value>>) -> Property {
    let list = list.into_iter().map(|l| l.into()).collect();
    Property::List(list)
}

mod tests {
    use super::{Property, property_value};

    #[test]
    fn test() {
        let t: String;
        let tp = property_value("test");
        t = tp.into();
    }
}
