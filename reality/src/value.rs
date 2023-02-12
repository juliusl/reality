use std::{
    cmp::Ordering,
    collections::{hash_map::DefaultHasher, BTreeSet},
    fmt::Display,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

/// Enumeration of possible attribute value types.
/// 
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum Value {
    #[default]
    Empty,
    Bool(bool),
    TextBuffer(String),
    Int(i32),
    IntPair(i32, i32),
    IntRange(i32, i32, i32),
    Float(f32),
    FloatPair(f32, f32),
    FloatRange(f32, f32, f32),
    BinaryVector(Vec<u8>),
    Reference(u64),
    Symbol(String),
    Complex(BTreeSet<String>),
}

impl Value {
    /// Returns the toml version of this value,
    /// 
    pub fn toml(&self) -> toml_edit::Item {
        use toml_edit::value;
        match self {
            Value::Empty => {
               value(".empty")
            },
            Value::Bool(b) => {
                value(format!(".bool {b}"))
            },
            Value::TextBuffer(t) => {
                value(format!(".text {t}"))
            },
            Value::Int(i) => {
                value(format!(".int {i}"))
            },
            Value::IntPair(a, b) => {
               value(format!(".int_pair {a}, {b}"))
            },
            Value::IntRange(a, b, c) => {
                value(format!(".int_range {a}, {b}, {c}"))
            },
            Value::Float(f) => {
                value(format!(".float {f}"))
            },
            Value::FloatPair(a, b) => {
                value(format!(".float_pair {a}, {b}"))
            },
            Value::FloatRange(a, b, c) => {
                value(format!(".float_range {a}, {b}, {c}"))
            },
            Value::BinaryVector(bin) => {
                value(format!(".bin {}", base64::encode(bin)))
            },
            Value::Reference(r) => {
                value(format!(".ref {r}"))
            },
            Value::Symbol(s) => {
                value(format!(".symbol {s}"))
            },
            Value::Complex(c) => {
               let c = c.iter().cloned().collect::<Vec<_>>();
               let c = c.join(", ");
               value(format!(".complex {c}"))
            },
        }
    }

    /// Returns an empty tuple if value is an Empty type,
    /// 
    pub fn empty(&self) -> Option<()> {
        match self {
            Self::Empty => Some(()),
            _ => None
        }
    }

    /// Returns a bool if this value is a bool literal,
    /// 
    pub fn bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None
        }
    }

    /// Returns a String if this value is a text buffer,
    /// 
    pub fn text(&self) -> Option<String> {
        match self {
            Self::TextBuffer(buffer) => Some(buffer.to_string()),
            _ => None
        }
    }

    /// Returns an i32 if this value is an int,
    /// 
    pub fn int(&self) -> Option<i32> {
        match self {
            Self::Int(i) => Some(*i), 
            _ => None 
        }
    }

    /// Returns an tuple (i32, i32) if this value is an int pair,
    /// 
    pub fn int_pair(&self) -> Option<(i32, i32)> {
        match self {
            Self::IntPair(a, b) => Some((*a, *b)), 
            _ => None,
        }
    }

    /// Returns a tuple (i32, i32, i32) if this value is an int range,
    /// 
    pub fn int_range(&self) -> Option<(i32, i32, i32)> {
        match self {
            Self::IntRange(a, b, c) => Some((*a, *b, *c)), 
            _ => None,
        }
    }

    /// Returns an f32 if this value is a float,
    /// 
    pub fn float(&self) -> Option<f32> {
        match self {
            Self::Float(a) => Some(*a),
            _ => None,
        }
    }

    /// Returns a tuple (f32, f32) if this value is a float pair, 
    /// 
    pub fn float_pair(&self) -> Option<(f32, f32)> {
        match self {
            Self::FloatPair(a, b) => Some((*a, *b)),
            _ => None,
        }
    }

    /// Returns a tuple (f32, f32, f32) if this value is a float range,
    /// 
    pub fn float_range(&self) -> Option<(f32, f32, f32)> {
        match self {
            Self::FloatRange(a, b, c) => Some((*a, *b, *c)),
            _ => None,
        }
    }


    /// Returns a STring if this value is a symbol,
    /// 
    pub fn symbol(&self) -> Option<String> {
        match self {
            Self::Symbol(symbol) => Some(symbol.to_string()),
            _ => None,
        }
    }
    
    /// Returns a vector of bytes if this values is a binary vector,
    /// 
    pub fn binary(&self) -> Option<Vec<u8>> {
        match self {
            Self::BinaryVector(vec) => Some(vec.to_vec()),
            _ => None, 
        }
    }

    /// Returns a btree set if this value is a complex,
    /// 
    pub fn complex(&self) -> Option<BTreeSet<String>> {
        match self {
            Self::Complex(c) => Some(c.clone()) ,
            _ => None,
        }
    }

    /// Returns value if self is a number type,
    /// 
    pub fn number(&self) -> Option<Value> {
        match self {
            Value::Int(_) |
            Value::IntPair(_, _) |
            Value::IntRange(_, _, _) |
            Value::Float(_) |
            Value::FloatPair(_, _) |
            Value::FloatRange(_, _, _) => Some(self.clone()),
            _ => {
                None
            }
        }
    }

    /// Converts to Value::Reference(),
    ///
    /// If self is already Value::Reference(), returns self w/o rehashing
    pub fn to_ref(&self) -> Value {
        Value::Reference(match self {
            Value::Reference(r) => *r,
            _ => {
                let state = &mut DefaultHasher::default();
                self.hash(state);
                state.finish()
            }
        })
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<BTreeSet<String>> for Value {
    fn from(b: BTreeSet<String>) -> Self {
        Value::Complex(b)
    }
}

impl From<&'static str> for Value {
    /// Symbols are typically declared in code
    ///
    fn from(s: &'static str) -> Self {
        Value::Symbol(s.to_string())
    }
}

impl From<usize> for Value {
    fn from(c: usize) -> Self {
        Value::Int(c as i32)
    }
}

impl Eq for Value {}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if let Some(ordering) = self.partial_cmp(other) {
            ordering
        } else {
            Ordering::Less
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Empty
            | Value::Symbol(_)
            | Value::Float(_)
            | Value::Int(_)
            | Value::Bool(_)
            | Value::TextBuffer(_)
            | Value::IntPair(_, _)
            | Value::FloatPair(_, _)
            | Value::FloatRange(_, _, _)
            | Value::IntRange(_, _, _) => {
                write!(f, "{:?}", self)?;
            }
            Value::BinaryVector(vec) => {
                write!(f, "{}", base64::encode(vec))?;
            }
            Value::Reference(_) => return write!(f, "{:?}", self),
            _ => {}
        }

        let r = self.to_ref();
        write!(f, "::{:?}", r)
    }
}

impl Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Float(f) => f.to_bits().hash(state),
            Value::Int(i) => i.hash(state),
            Value::Bool(b) => b.hash(state),
            Value::FloatRange(f, fm, fmx) => {
                f.to_bits().hash(state);
                fm.to_bits().hash(state);
                fmx.to_bits().hash(state);
            }
            Value::IntRange(i, im, imx) => {
                i.hash(state);
                im.hash(state);
                imx.hash(state);
            }
            Value::TextBuffer(txt) => txt.hash(state),
            Value::Empty => {}
            Value::IntPair(i1, i2) => {
                i1.hash(state);
                i2.hash(state);
            }
            Value::FloatPair(f1, f2) => {
                f1.to_bits().hash(state);
                f2.to_bits().hash(state);
            }
            Value::BinaryVector(v) => {
                v.hash(state);
            }
            Value::Reference(r) => r.hash(state),
            Value::Symbol(r) => r.hash(state),
            Value::Complex(r) => r.hash(state),
        };
    }
}
