use crate::{Value, BlockProperties};
use std::{fmt::Display, sync::Arc};

/// Enumeration of property types
///
#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockProperty {
    /// Property is a single value
    Single(Value),
    /// Property is a list of values
    List(Vec<Value>),
    /// Property is a read-only reference to block properties,
    Properties(Arc<BlockProperties>),
    /// Reverse property that indicates this property name is required
    #[deprecated]
    Required(Option<Value>),
    /// Reverse property that indiciates this property name is required
    #[deprecated]
    Optional(Option<Value>),
    /// Indicates that this block property is currently empty
    Empty,
}

impl BlockProperty {
    /// Returns true if the property is a bool value and true
    ///
    pub fn is_enabled(&self) -> bool {
        match self {
            BlockProperty::Single(Value::Bool(enabled)) => *enabled,
            _ => false,
        }
    }

    /// Returns a string if the property is a single text buffer
    ///
    pub fn text(&self) -> Option<&String> {
        match self {
            BlockProperty::Single(Value::TextBuffer(text)) => Some(text),
            _ => None,
        }
    }

    /// Returns a string if the property is a single symbol
    ///
    pub fn symbol(&self) -> Option<&String> {
        match self {
            BlockProperty::Single(Value::Symbol(symbol)) => Some(symbol),
            _ => None,
        }
    }

    /// Returns an integer if the property is an int,
    ///
    pub fn int(&self) -> Option<i32> {
        match self {
            BlockProperty::Single(Value::Int(i)) => Some(*i),
            _ => None,
        }
    }

    /// Returns a vector of strings if the property is a single text buffer,
    /// or if the property is a list of values, filters all text buffers
    ///
    pub fn text_vec(&self) -> Option<Vec<String>> {
        match self {
            BlockProperty::Single(Value::TextBuffer(text)) => Some(vec![text.to_string()]),
            BlockProperty::List(values) => Some(
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
    pub fn symbol_vec(&self) -> Option<Vec<String>> {
        match self {
            BlockProperty::Single(Value::Symbol(text)) => Some(vec![text.to_string()]),
            BlockProperty::List(values) => Some(
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
    pub fn int_vec(&self) -> Option<Vec<i32>> {
        match self {
            BlockProperty::Single(Value::Int(int)) => Some(vec![*int]),
            BlockProperty::List(values) => Some(
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
    pub fn float_vec(&self) -> Option<Vec<f32>> {
        match self {
            BlockProperty::Single(Value::Float(float)) => Some(vec![*float]),
            BlockProperty::List(values) => Some(
                values
                    .iter()
                    .filter_map(Value::float)
                    .collect::<Vec<_>>(),
            ),
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
            BlockProperty::Single(single) => on_single(single),
            BlockProperty::List(list) => on_list(list.as_mut()),
            BlockProperty::Properties(_) => {
                // read-only
                return;
            }
            BlockProperty::Empty => match on_empty() {
                Some(value) => *self = BlockProperty::Single(value),
                None => {}
            },
            BlockProperty::Optional(default_value) | BlockProperty::Required(default_value) => {
                match on_empty() {
                    Some(value) => *self = BlockProperty::Single(value),
                    None => match default_value {
                        Some(value) => *self = BlockProperty::Single(value.clone()),
                        None => {}
                    },
                }
            }
        }
    }
}

impl Default for BlockProperty {
    fn default() -> Self {
        BlockProperty::Empty
    }
}

impl Display for BlockProperty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockProperty::Single(single) => display_value(f, single),
            BlockProperty::List(list) => {
                for val in list {
                    display_value(f, val)?;
                    write!(f, ", ")?;
                }
                Ok(())
            }
            BlockProperty::Properties(_) =>  write!(f, "properties - todo display"),
            BlockProperty::Required(_) => write!(f, "required, value is not set"),
            BlockProperty::Optional(_) => write!(f, "optional, value is not set"),
            BlockProperty::Empty => write!(f, "empty value"),
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
