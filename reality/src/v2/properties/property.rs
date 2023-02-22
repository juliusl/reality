use crate::Value;
use std::sync::Arc;
use std::fmt::Display;

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
            _ => false,
        }
    }

    /// Returns a string if the property is a single text buffer
    ///
    pub fn as_text(&self) -> Option<&String> {
        match self {
            Property::Single(Value::TextBuffer(text)) => Some(text),
            _ => None,
        }
    }

    /// Returns a string if the property is a single symbol
    ///
    pub fn as_symbol(&self) -> Option<&String> {
        match self {
            Property::Single(Value::Symbol(symbol)) => Some(symbol),
            _ => None,
        }
    }

    /// Returns an integer if the property is an int,
    ///
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Property::Single(Value::Int(i)) => Some(*i),
            _ => None,
        }
    }

    /// Returns a vector of strings if the property is a single text buffer,
    /// or if the property is a list of values, filters all text buffers
    ///
    pub fn as_text_vec(&self) -> Option<Vec<String>> {
        match self {
            Property::Single(Value::TextBuffer(text)) => Some(vec![text.to_string()]),
            Property::List(values) => Some(
                values
                    .iter()
                    .filter_map(Value::text)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    /// Returns a vector of strings if the property is a single symbol,
    /// or if the property is a list of values, filters all symbols
    ///
    pub fn as_symbol_vec(&self) -> Option<Vec<String>> {
        match self {
            Property::Single(Value::Symbol(text)) => Some(vec![text.to_string()]),
            Property::List(values) => Some(
                values
                    .iter()
                    .filter_map(Value::symbol)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    /// Returns a vector of integers if the property is a single int,
    /// or if the property is a list of values, filters all ints
    ///
    pub fn as_int_vec(&self) -> Option<Vec<i32>> {
        match self {
            Property::Single(Value::Int(int)) => Some(vec![*int]),
            Property::List(values) => Some(
                values
                    .iter()
                    .filter_map(Value::int)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    /// Returns a vector of integers if the property is a single int,
    /// or if the property is a list of values, filters all ints
    ///
    pub fn as_float_vec(&self) -> Option<Vec<f32>> {
        match self {
            Property::Single(Value::Float(float)) => Some(vec![*float]),
            Property::List(values) => Some(
                values
                    .iter()
                    .filter_map(Value::float)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    /// Returns a vector of values,
    /// 
    pub fn as_value_vec(&self) -> Option<&Vec<Value>> {
        match self {
            Property::List(list) => {
                Some(list)
            },
            _ => {
                None
            }
        }
    }

    /// Returns as properties,
    /// 
    pub fn as_properties(&self) -> Option<Arc<Properties>> {
        match self {
            Property::Properties(properties) => Some(properties.clone()),
            _ => {
                None
            }
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
            Property::Properties(_) =>  write!(f, "properties - todo display"),
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
