use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::Value;

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

mod v2 {
    use std::{collections::HashMap, sync::Arc};

    use specs::{Entity, EntityBuilder, World, WorldExt};
    use toml_edit::{Document, Item};

    use crate::Value;

    #[derive(Default)]
    pub struct ExtensionTable {
        table: HashMap<u64, Arc<dyn ExtensionAction>>,
    }

    /// V2 version of Attribute type,
    ///
    pub struct Attribute {
        /// Identifier string,
        ///
        pub ident: String,
        /// Value of this attribute,
        ///
        pub value: Value,
        /// Stack of actions that will be applied to this attribute during it's transient phase,
        ///
        action_stack: Vec<Action>,
    }

    impl Attribute {
        /// Applies this attribute to a world,
        ///
        pub fn apply(&mut self, doc: &Document, world: &mut World) -> Result<(), Error> {
            for a in self.action_stack.iter() {
                match a {
                    Action::Define => todo!(),
                    Action::Extension(_) => {
                        todo!()
                    },
                    Action::With(_, _) => todo!(),
                    Action::BuildDocument(build_doc) => {
                        let _built = build_doc(doc, world.create_entity())?;
                        // TODO: This would be appened to the root of this attribute somehow
                    },
                }
            }

            Ok(())
        }
    }

    /// Enumeration of attribute actions that apply during the transient phase of the attribute's lifecycle,
    ///
    #[derive(Default)]
    pub enum Action {
        /// This action will define a property value on the attribute's entity using the current state,
        ///
        #[default]
        Define,
        /// This action will lookup an extension action by name to apply to the attribute's entity,
        ///
        /// If no extension action is found, it will be skipped
        Extension(String),
        /// This action will define a property value on the attribute's entity,
        ///
        With(String, Value),
        /// This action will build an entity,
        ///
        BuildDocument(BuildDocumentFunc),
    }

    /// Trait for an extension action,
    ///
    pub trait ExtensionAction
    where
        Self: Send + Sync,
    {
        /// Identifier that represents this extension action,
        ///
        fn ident(self: Arc<Self>) -> String;

        /// Expand the current action into a stack of actions to apply,
        ///
        fn expand(self: Arc<Self>, attribute: &Attribute) -> Vec<Action>;
    }

    /// Returns an action that will apply a property,
    ///
    pub fn with(name: impl Into<String>, value: impl Into<Value>) -> Action {
        Action::With(name.into(), value.into())
    }

    /// Returns an action that will apply an attribute as a property,
    ///
    pub fn define() -> Action {
        Action::Define
    }

    /// Returns an extension action,
    ///
    pub fn extension(ident: impl Into<String>) -> Action {
        Action::Extension(ident.into())
    }

    /// Returns an action that builds an entity from a document,
    ///
    pub fn build_document(func: BuildDocumentFunc) -> Action {
        Action::BuildDocument(func)
    }

    pub trait ValueProvider<'a>
    where
        Self: core::ops::Index<&'a str, Output = Item>,
    {
        /// Returns a result that contains an integer, otherwise returns an error,
        ///
        fn int(&'a self, ident: &'static str) -> Result<i64, Error> {
            self.find(ident, toml_edit::Value::as_integer)
        }

        /// Returns a result that contains an integer, otherwise returns an error,
        ///
        fn float(&'a self, ident: &'static str) -> Result<f64, Error> {
            self.find(ident, toml_edit::Value::as_float)
        }

        /// Returns a result that contains a string, otherwise returns an error,
        ///
        fn string(&'a self, ident: &'static str) -> Result<&str, Error> {
            self.find(ident, toml_edit::Value::as_str)
        }

        /// Returns a result that contains a boolean, otherwise returns an error,
        ///
        fn bool(&'a self, ident: &'static str) -> Result<bool, Error> {
            self.find(ident, toml_edit::Value::as_bool)
        }

        /// Finds a value, mapping it w/ map, otherwise constructs a proper error,
        ///
        fn find<T>(
            &'a self,
            ident: &'static str,
            map: fn(&'a toml_edit::Value) -> Option<T>,
        ) -> Result<T, Error> {
            if let Some(i) = self[ident].as_value().and_then(map) {
                Ok(i)
            } else {
                Err(Error {
                    document_item: Some(self["src"].clone()),
                })
            }
        }
    }

    impl ValueProvider<'_> for Item {}

    /// Function type for converting a toml document into an entity w/ components,
    ///
    pub type BuildDocumentFunc = fn(&Document, EntityBuilder) -> Result<Entity, Error>;

    /// Struct for build errors,
    ///
    pub struct Error {
        /// If this error is related to document state, this item will contain additional information,
        ///
        document_item: Option<Item>,
    }

    mod tests {
        use std::sync::Arc;

        use specs::{Builder, Component, VecStorage};
        use toml_edit::Value;

        use super::{build_document, ExtensionAction, ExtensionTable, ValueProvider};

        #[derive(Component)]
        #[storage(VecStorage)]
        struct Pos(usize, usize);

        struct Test;

        impl ExtensionAction for Test {
            fn ident(self: std::sync::Arc<Self>) -> String {
                "test".to_string()
            }

            fn expand(self: std::sync::Arc<Self>, _: &super::Attribute) -> Vec<super::Action> {
                vec![build_document(|d, eb| {
                    let x = d["test"].int("x")?;
                    let y = d["test"].int("y")?;

                    let eb = eb.with(Pos(x as usize, y as usize));

                    Ok(eb.build())
                })]
            }
        }

        #[test]
        fn test() {
            let mut table = ExtensionTable::default();
            table.table.insert(0, Arc::new(Test {}));

            let t = table.table.get(&0).map(|t| t.clone().ident());
        }
    }
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
