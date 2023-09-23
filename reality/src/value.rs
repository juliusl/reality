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
    /// Returns an empty tuple if value is an Empty type,
    ///
    pub fn empty(&self) -> Option<()> {
        match self {
            Self::Empty => Some(()),
            _ => None,
        }
    }

    /// Returns a bool if this value is a bool literal,
    ///
    pub fn bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns a String if this value is a text buffer,
    ///
    pub fn text(&self) -> Option<String> {
        match self {
            Self::TextBuffer(buffer) => Some(buffer.to_string()),
            _ => None,
        }
    }

    /// Returns an i32 if this value is an int,
    ///
    pub fn int(&self) -> Option<i32> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
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
            Self::Complex(c) => Some(c.clone()),
            _ => None,
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

impl From<usize> for Value {
    fn from(c: usize) -> Self {
        Value::Int(c as i32)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Int(value)
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

pub mod v2 {
    use std::collections::BTreeSet;
    use std::convert::Infallible;
    use std::marker::PhantomData;
    use std::str::FromStr;

    use crate::attributes::Container;
    use crate::AsyncStorageTarget;
    use crate::AttributeType;
    use crate::Dispatcher;
    use crate::StorageTarget;

    /// Struct for a value container,
    ///
    pub struct ValueContainer<T>(PhantomData<T>);

    /// Split and parse a comma-seperated list,
    ///
    fn from_comma_sep<T>(input: &str) -> Vec<T>
    where
        T: FromStr,
    {
        input
            .split_terminator(',')
            .filter_map(|i| i.trim().parse().ok())
            .collect()
    }

    macro_rules! impl_attr {
        ($ident:literal, $ty:ty, $default:literal) => {
            impl<S: StorageTarget<Attribute = crate::Attribute>> AttributeType<S>
                for ValueContainer<$ty>
            {
                fn ident() -> &'static str {
                    $ident
                }

                fn parse(parser: &mut crate::AttributeParser<S>, content: impl AsRef<str>) {
                    if let Some(v) = content.as_ref().parse::<$ty>().ok() {
                        parser.set_edit(v);
                    } else {
                        parser.set_edit($default);
                    }
                }
            }
        };
        ($ident:literal, $ty:ty, $default:literal, $value_ty:ident) => {
            impl<S: StorageTarget<Attribute = crate::Attribute>> AttributeType<S>
                for ValueContainer<$ty>
            {
                fn ident() -> &'static str {
                    $ident
                }

                fn parse(parser: &mut crate::AttributeParser<S>, content: impl AsRef<str>) {
                    if let Some(v) = content.as_ref().parse::<$ty>().ok() {
                        parser.set_edit(v);
                    } else {
                        parser.set_edit($default);
                    }
                }
            }

            impl From<$ty> for super::Value {
                fn from(value: $ty) -> Self {
                    super::Value::$value_ty(value)
                }
            }
        };
        ($ident:literal, $ty:ty, $default:literal, $value_ty:ident, $into_ty:ty) => {
            impl<S: StorageTarget<Attribute = crate::Attribute>> AttributeType<S>
                for ValueContainer<$ty>
            {
                fn ident() -> &'static str {
                    $ident
                }

                fn parse(parser: &mut crate::AttributeParser<S>, content: impl AsRef<str>) {
                    if let Some(v) = content.as_ref().parse::<$into_ty>().ok() {
                        parser.set_edit(v);
                    } else {
                        parser.set_edit($default);
                    }
                }
            }

            impl From<$ty> for super::Value {
                fn from(value: $ty) -> Self {
                    super::Value::$value_ty(<$ty as Into<$into_ty>>::into(value))
                }
            }
        };
    }

    macro_rules! impl_attr_pair {
        ($ident:literal, $ty:ty, $default:literal, $value_ty:ident, $into_ty:ty) => {
            impl<S: StorageTarget<Attribute = crate::Attribute>> AttributeType<S>
                for ValueContainer<$ty>
            {
                fn ident() -> &'static str {
                    $ident
                }

                fn parse(parser: &mut crate::AttributeParser<S>, content: impl AsRef<str>) {
                    if let [v0, v1, ..] = from_comma_sep::<$into_ty>(content.as_ref())[..2] {
                        let pair = super::Value::$value_ty(v0, v1);
                        parser.set_edit(pair);
                    } else {
                        parser.set_edit(super::Value::$value_ty($default, $default));
                    }
                }
            }
        };
    }

    macro_rules! impl_attr_range {
        ($ident:literal, $ty:ty, $default:literal, $value_ty:ident, $into_ty:ty) => {
            impl<S: StorageTarget<Attribute = crate::Attribute>> AttributeType<S>
                for ValueContainer<$ty>
            {
                fn ident() -> &'static str {
                    $ident
                }

                fn parse(parser: &mut crate::AttributeParser<S>, content: impl AsRef<str>) {
                    if let [v0, v1, v2, ..] = from_comma_sep::<$into_ty>(content.as_ref())[..2] {
                        let range = super::Value::$value_ty(v0, v1, v2);
                        parser.set_edit(range);
                    } else {
                        parser.set_edit(super::Value::$value_ty($default, $default, $default));
                    }
                }
            }
        };
    }

    impl_attr!("bool", bool, false);
    impl_attr!("int", i32, 0i32);
    impl_attr_pair!("int2", [i32; 2], 0i32, IntPair, i32);
    impl_attr_range!("int3", [i32; 3], 0i32, IntRange, i32);
    impl_attr!("float", f32, 0f32, Float);
    impl_attr_pair!("float2", [f32; 2], 0f32, FloatPair, f32);
    impl_attr_range!("float3", [f32; 3], 0f32, FloatRange, f32);
    impl_attr!("text", String, "", TextBuffer);
    impl_attr!("symbol", &str, "", Symbol, String);

    impl<S: StorageTarget<Attribute = crate::Attribute>> AttributeType<S> for ValueContainer<Vec<u8>> {
        fn ident() -> &'static str {
            "bin"
        }

        fn parse(parser: &mut crate::AttributeParser<S>, content: impl AsRef<str>) {
            let binary = match base64::decode(content.as_ref()) {
                Ok(content) => super::Value::BinaryVector(content),
                Err(_) => super::Value::BinaryVector(vec![]),
            };
            parser.set_value(binary);
        }
    }

    impl<S: StorageTarget<Attribute = crate::Attribute>> AttributeType<S>
        for ValueContainer<BTreeSet<String>>
    {
        fn ident() -> &'static str {
            "complex"
        }

        fn parse(parser: &mut crate::AttributeParser<S>, content: impl AsRef<str>) {
            let binary = match base64::decode(content.as_ref()) {
                Ok(content) => super::Value::BinaryVector(content),
                Err(_) => super::Value::BinaryVector(vec![]),
            };
            parser.set_value(binary);
        }
    }

    /// Container for an attribute type w/ a dedicated dispatcher,
    ///
    pub struct Attribute<
        S: StorageTarget,
        V: AttributeType<S>
            + Unpin
            + Default
            + Send
            + Sync
            + Clone
            + std::fmt::Debug
            + From<std::string::String>
            + 'static,
    > {
        /// Value,
        ///
        value: V,
        /// Next value,
        ///
        next: Option<V>,
        /// Dispatcher to dispatch state changes for this attribute,
        ///
        dispatcher: Option<Dispatcher<S, V>>,
    }

    impl<
            S: StorageTarget + 'static,
            V: AttributeType<S>
                + Unpin
                + Default
                + Send
                + Sync
                + Clone
                + std::fmt::Debug
                + From<std::string::String>
                + 'static,
        > Clone for Attribute<S, V>
    {
        fn clone(&self) -> Self {
            Self {
                value: self.value.clone(),
                next: self.next.clone(),
                dispatcher: self.dispatcher.clone(),
            }
        }
    }

    impl<S, V> Container for Attribute<S, V>
    where
        S: StorageTarget + 'static,
        V: AttributeType<S>
            + Unpin
            + Default
            + Send
            + Sync
            + Clone
            + std::fmt::Debug
            + From<std::string::String>
            + 'static,
        Self: TryFrom<Option<Attribute<S, V>>>
    {
        type Id = u32;

        type Label = String;

        type Value = V;

        type Created = Option<Self>;

        type Committed = Option<Self>;

        fn create(id: Self::Id, ty: impl Into<Self::Label>) -> Self::Created {
            todo!()
        }

        fn set_value(&mut self, value: Self::Value) -> Option<Self::Value> {
            todo!()
        }

        fn value(&self) -> &Self::Value {
            todo!()
        }

        fn id(&self) -> Self::Id {
            todo!()
        }

        fn pending(&self) -> Option<&Self::Value> {
            todo!()
        }

        fn get_label(&self, name: &str) -> Option<Self::Label> {
            todo!()
        }

        fn set_label(&mut self, name: &str, label: impl Into<Self::Label>) -> Option<Self::Label> {
            todo!()
        }

        fn labels(&self) -> Vec<(&str, Self::Label)> {
            todo!()
        }

        fn edit(&mut self, value: Self::Value) {
            todo!()
        }

        fn commit(&self) -> Self::Committed {
            todo!()
        }
    }
}
