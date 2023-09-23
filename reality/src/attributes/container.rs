use std::collections::BTreeSet;
use std::convert::Infallible;
use std::str::FromStr;

use crate::{Attribute, Value};

/// Trait for types that act as a container for a value,
///
pub trait Container: Clone + Sized {
    /// Identifier type that can be used to map and uniquely identify a container type,
    /// 
    type Id: std::ops::AddAssign<u32>
        + std::ops::Sub<u32, Output = u32>
        + From<u32>
        + Default
        + Ord
        + Eq
        + std::hash::Hash
        + std::fmt::Debug
        + Copy
        + Send
        + Sync
        + 'static;

    /// Type used as labels for the container,
    ///
    type Label: std::fmt::Debug + FromStr<Err = Infallible> + AsRef<str>;

    /// Value this container contains,
    ///
    type Value: Unpin + Default + Send + Sync + Clone + std::fmt::Debug + TryFrom<Self::Label>;

    /// Wrapper-type returned when this container is first created,
    ///
    type Created: Clone + Sized + TryInto<Self>;

    /// Wrapper-type returned when this container is committed,
    ///
    type Committed: Clone + Sized + TryInto<Self>;

    /// Creates a new container w/ id and provided type,
    ///
    fn create(id: Self::Id, ty: impl Into<Self::Label>) -> Self::Created;

    /// Set the value of this container,
    ///
    /// Return the previous value if one was set,
    ///
    fn set_value(&mut self, value: Self::Value) -> Option<Self::Value>;

    /// Returns the current value,
    ///
    fn value(&self) -> &Self::Value;

    /// Returns the id of this value,
    ///
    fn id(&self) -> Self::Id;

    /// Returns the pending value if it exists,
    ///
    fn pending(&self) -> Option<&Self::Value>;

    /// Gets a label by name,
    ///
    fn get_label(&self, name: &str) -> Option<Self::Label>;

    /// Sets a label by name,
    ///
    fn set_label(&mut self, name: &str, label: impl Into<Self::Label>) -> Option<Self::Label>;

    /// Returns a map of all labels this container has,
    ///
    fn labels(&self) -> Vec<(&str, Self::Label)>;

    /// Edit the value of this container,
    ///
    /// **Note** The implementation should not persist the change until `commit(..)` is called,
    ///
    fn edit(&mut self, value: Self::Value);

    /// Commit any pending values and return the commit wrapper,
    ///
    fn commit(&self) -> Self::Committed;

    /// Create a new value container w/ id and type,
    ///
    /// Returns the new container if successful, otherwise returns None.
    ///
    /// **Note** If the name is not set, than the implementation can assume that `ty` is the name,
    ///
    fn new(
        id: Self::Id,
        ty: impl AsRef<str>,
        name: Option<&str>,
        value: Option<Self::Value>,
    ) -> Option<Self> {
        let ty: Self::Label = ty.as_ref().parse().expect("should be infallible");

        let new = Self::create(id, ty);

        if let Some(mut new) = new.try_into().ok() {
            if let Some(name) = name.and_then(|n| n.parse::<Self::Label>().ok()) {
                new.set_label("name", name);
            }

            if let Some(value) = value {
                new.set_value(value);
            }
            Some(new)
        } else {
            None
        }
    }

    /// Set the parent of this container,
    ///
    /// Return the previous parent if one was set,
    ///
    /// **Note**: Implementing this is opt-in because to allow the flexibility for a parent/child relationship between containers to exist
    ///
    #[allow(unused_variables)]
    fn set_parent(&mut self, id: Self::Id) -> Option<Self::Id> {
        None
    }

    /// Returns the parent of this container,
    ///
    fn parent(&self) -> Option<Self::Id> {
        None
    }

    /// Set the name of this container,
    ///
    /// Return the previous name if one was set,
    ///
    fn set_name(&mut self, name: Self::Label) -> Option<Self::Label> {
        self.set_label("name", name)
    }

    /// Casts the inner type by creating a new container of the desired-type and transferring
    /// over labels and value.
    ///
    /// Returns the previous container if casting was successful, otherwise returns None and is a no-op
    ///
    fn cast_into(&mut self, ty: Self::Label) -> Option<Self> {
        let casted = Self::create(self.id(), ty);
        if let Some(mut next) = TryInto::<Self>::try_into(casted).ok() {
            let labels = self.labels();
            for (name, label) in labels {
                next.set_label(name, label);
            }
            let last = self.clone();
            *self = next;
            Some(last)
        } else {
            None
        }
    }
}

impl Container for Attribute {
    type Id = u32;

    type Label = Label;

    type Value = Value;

    type Created = Option<Attribute>;

    type Committed = Option<Attribute>;

    fn create(id: Self::Id, ty: impl Into<Self::Label>) -> Self::Created {
        let ty = ty.into();
        Some(Attribute {
            id,
            name: format!("{}", ty.as_ref()),
            value: Value::try_from(ty).unwrap_or(Value::Empty),
            transient: None,
        })
    }

    fn set_value(&mut self, value: Self::Value) -> Option<Self::Value> {
        if self.value.empty().is_none() {
            let last = self.value.clone();
            self.value = value;
            Some(last)
        } else {
            None
        }
    }

    #[inline]
    fn value(&self) -> &Self::Value {
        &self.value
    }

    #[inline]
    fn id(&self) -> Self::Id {
        self.id
    }

    #[inline]
    fn pending(&self) -> Option<&Self::Value> {
        self.transient.as_ref().map(|(_, value)| value)
    }

    fn get_label(&self, name: &str) -> Option<Self::Label> {
        match name {
            "name" => Some(Label(self.name.to_string())),
            "ty" => {
                let literal = Label(
                    match self.value {
                        Value::Empty => "empty",
                        Value::Bool(_) => "bool",
                        Value::TextBuffer(_) => "text",
                        Value::Int(_) => "int",
                        Value::IntPair(_, _) => "int2",
                        Value::IntRange(_, _, _) => "int3",
                        Value::Float(_) => "float",
                        Value::FloatPair(_, _) => "float2",
                        Value::FloatRange(_, _, _) => "float3",
                        Value::BinaryVector(_) => "bin",
                        Value::Reference(_) => "reference",
                        Value::Symbol(_) => "symbol",
                        Value::Complex(_) => "complex",
                    }
                    .to_string(),
                );

                // If a '::' is present then the symbol is actually the type,
                // otherwise the ty is the literal value type
                //
                if let Some((_, symbol)) = self.name.split_once("::") {
                    Some(Label(symbol.to_string()))
                } else {
                    Some(literal)
                }
            }
            "symbol" => {
                if let Some((_, symbol)) = self.name.split_once("::") {
                    Some(Label(symbol.to_string()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn set_label(&mut self, name: &str, label: impl Into<Self::Label>) -> Option<Self::Label> {
        match name {
            "name" => {
                let last = Some(Label(self.name.clone()));
                self.name = label.into().0;
                last
            }
            "symbol" => {
                if let Some((name, _)) = self.name.split_once("::") {
                    let last = self.name.clone();
                    self.name = format!("{name}::{}", label.into().as_ref());
                    Some(Label(last))
                } else {
                    self.name = format!("{}::{}", self.name, label.into().as_ref());
                    None
                }
            }
            "ty" => self.cast_into(label.into()).and_then(|l| l.get_label("ty")),
            _ => None,
        }
    }

    fn labels(&self) -> Vec<(&str, Self::Label)> {
        vec![
            ("name", Label(self.name.clone())),
            (
                "symbol",
                if let Some((_, symbol)) = self.name.split_once("::") {
                    Label(symbol.to_string())
                } else {
                    Label::default()
                },
            ),
            ("ty", self.get_label("ty").unwrap_or_default()),
        ]
    }

    fn edit(&mut self, value: Self::Value) {
        self.edit((String::default(), value));
    }

    fn commit(&self) -> Self::Committed {
        let mut committing = self.clone();
        Attribute::commit(&mut committing);
        Some(committing)
    }
}

impl TryFrom<Option<Attribute>> for Attribute {
    type Error = ();

    fn try_from(value: Option<Attribute>) -> Result<Self, Self::Error> {
        if let Some(value) = value {
            Ok(value)
        } else {
            Err(())
        }
    }
}

#[derive(Debug, Default)]
pub struct Label(String);

impl AsRef<str> for Label {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Label> for Value {
    fn from(value: Label) -> Self {
        match value.as_ref() {
            "bool" => Value::Bool(false),
            "text" => Value::TextBuffer(String::new()),
            "int" => Value::Int(0),
            "int2" => Value::IntPair(0, 0),
            "int3" => Value::IntRange(0, 0, 0),
            "float" => Value::Float(0.0),
            "float2" => Value::FloatPair(0.0, 0.0),
            "float3" => Value::FloatRange(0.0, 0.0, 0.0),
            "bin" => Value::BinaryVector(vec![]),
            "reference" => Value::Reference(0),
            "symbol" => Value::Symbol(String::new()),
            "complex" => Value::Complex(BTreeSet::new()),
            _ => Value::Empty,
        }
    }
}

impl FromStr for Label {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Label(s.to_string()))
    }
}

#[test]
fn test() {
    let a = 99u64;
    let b = 108u64;
    let c = 34u64;
    let ab = a ^ b;
    let abc =  a ^ b ^ c;
    println!("{:b} ^ {:b} ^ {:b} = {:b} ({})", a, b, c, abc, abc);

    let _ab = abc ^ c;
    println!("{:b} ({}) == {:b} ({})", ab, ab, _ab, _ab);
}