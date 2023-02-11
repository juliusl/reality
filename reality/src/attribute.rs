use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::Value;

pub mod v2;

/// Struct for containing a name/value pair in either a stable or transient state,
///
/// # Background
///
/// An attribute is a value with a name and an owner. The owner is identified by an integer id.
///
/// An attribute can be in either two states, stable or transient. Stable means that the transient property has no value.
///
/// Transient means that the transient property of the attribute has a value. If this struct is serialized,
/// the transient property is never serialized with the attribute by default.
///
/// This property is useful to distinguish between data that is "in motion" and static data.
///
#[derive(Clone, Default, Debug, Serialize, Deserialize, Hash)]
pub struct Attribute {
    /// An id that points to the owner of this attribute, likely a specs entity id,
    ///
    pub id: u32,
    /// The name of this attribute, identifies the purpose of the value,
    ///
    pub name: String,
    /// The value of this attribute,
    ///
    pub value: Value,
    /// This is the transient portion of the attribute. Its state can change independent of the main
    /// attribute. It's usages are left intentionally undefined, but the most basic use case
    /// is mutating either the name or value of this attribute.
    ///
    /// For example, if this attribute was being used in a gui to represent form data,
    /// mutating the name or value directly might have unintended side-effects. However,
    /// editing the transient portion should not have any side effects, as long as the consumer
    /// respects that the state of this property is transient. Then if a change is to be comitted to this attribute,
    /// then commit() can be called to consume the transient state, and mutate the name/value, creating a new attribute.
    ///
    /// This is just one example of how this is used, but other protocols can also be defined.
    ///
    #[serde(skip)]
    pub transient: Option<(String, Value)>,
}

impl Ord for Attribute {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.id, &self.name, &self.value, &self.transient).cmp(&(
            other.id,
            &other.name,
            &other.value,
            &self.transient,
        ))
    }
}

impl Eq for Attribute {}

impl PartialEq for Attribute {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.value == other.value
            && self.transient == other.transient
    }
}

impl PartialOrd for Attribute {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (self.id, &self.name, &self.value, &self.transient).partial_cmp(&(
            other.id,
            &other.name,
            &other.value,
            &self.transient,
        ))
    }
}

impl Display for Attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#010x}::", self.id)?;
        write!(f, "{}::", self.name)?;

        Ok(())
    }
}

impl Into<(String, Value)> for &mut Attribute {
    fn into(self) -> (String, Value) {
        (self.name().to_string(), self.value().clone())
    }
}

impl Attribute {
    pub fn new(id: u32, name: impl Into<String>, value: Value) -> Attribute {
        Attribute {
            id,
            name: { name.into() },
            value,
            transient: None,
        }
    }

    /// Returns `true` when this attribute is in a `stable` state.
    /// A `stable` state means that there are no pending changes focused on this instance of the `attribute`.
    pub fn is_stable(&self) -> bool {
        self.transient.is_none()
    }

    /// Returns the transient part of this attribute
    pub fn transient(&self) -> Option<&(String, Value)> {
        self.transient.as_ref()
    }

    pub fn take_transient(&mut self) -> Option<(String, Value)> {
        self.transient.take()
    }

    pub fn commit(&mut self) {
        if let Some((name, value)) = &self.transient {
            self.name = name.clone();
            self.value = value.clone();
            self.transient = None;
        }
    }

    pub fn edit_self(&mut self) {
        let init = self.into();
        self.edit(init);
    }

    pub fn edit(&mut self, edit: (String, Value)) {
        self.transient = Some(edit);
    }

    pub fn edit_as(&mut self, edit: Value) {
        if let Some((name, _)) = &self.transient {
            self.transient = Some((name.to_string(), edit));
        } else {
            self.transient = Some((self.name().to_string(), edit));
        }
    }

    pub fn reset_editing(&mut self) {
        if let Some((name, value)) = &mut self.transient {
            *value = self.value.clone();
            *name = self.name.clone();
        }
    }

    // sets the id/owner of this attribute
    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    /// read the name of this attribute
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// read the current value of this attribute
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// write to the current value of this attribute
    pub fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    /// read the current id of this attribute
    /// This id is likely the entity owner of this attribute
    pub fn id(&self) -> u32 {
        self.id
    }
}
