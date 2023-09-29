use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::trace;

use runmd::prelude::*;

use super::attribute_type::OnParseField;
use super::attribute_type::ParsableAttributeTypeField;
use super::attribute_type::ParsableField;
use super::AttributeTypeParser;
use super::StorageTarget;
use crate::AttributeType;

/// Resource for storing attribute types,
///
#[derive(Clone)]
pub struct AttributeTypePackage<S: StorageTarget>(HashSet<AttributeTypeParser<S>>);

/// Maintains attribute types and matches runmd nodes to the corresponding attribute type parser,
///
#[derive(Default)]
pub struct AttributeParser<Storage: StorageTarget> {
    /// Current name being parsed,
    ///
    name: Option<String>,
    /// Current tag being parsed,
    ///
    tag: Option<String>,
    /// Table of attribute type parsers,
    ///
    attribute_types: HashMap<String, AttributeTypeParser<Storage>>,
    /// Reference to centralized-storage,
    ///
    storage: Option<Arc<std::sync::RwLock<Storage>>>,
}

impl<S: StorageTarget> Clone for AttributeParser<S> {
    fn clone(&self) -> Self {
        Self {
            tag: self.tag.clone(),
            name: self.name.clone(),
            attribute_types: self.attribute_types.clone(),
            storage: self.storage.clone(),
        }
    }
}

impl<S: StorageTarget> std::fmt::Debug for AttributeParser<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttributeParser")
            .field("name", &self.tag)
            .field("symbol", &self.name)
            .field("attribute_table", &self.attribute_types)
            .field("storage", &self.storage.is_some())
            .finish()
    }
}

impl<S: StorageTarget> AttributeParser<S> {
    /// Adds a custom attribute parser and returns self,
    ///
    pub fn with_type<C>(&mut self) -> &mut Self
    where
        C: AttributeType<S>,
    {
        self.add_type(AttributeTypeParser::new::<C>());
        self
    }

    /// Adds a custom attribute parser,
    ///
    pub fn add_type(&mut self, custom_attr: impl Into<AttributeTypeParser<S>>) {
        let custom_attr = custom_attr.into();
        self.attribute_types
            .insert(custom_attr.ident().to_string(), custom_attr);
    }

    /// Adds a custom attribute parser,
    ///
    /// Returns a clone of the custom attribute added,
    ///
    pub fn add_type_with(
        &mut self,
        ident: impl AsRef<str>,
        parse: fn(&mut AttributeParser<S>, String),
    ) -> AttributeTypeParser<S> {
        let attr = AttributeTypeParser::new_with(ident, parse);
        self.add_type(attr.clone());
        attr
    }

    /// Returns attribute parser with a parseable type, chainable
    ///
    pub fn with_parseable_field<const FIELD_OFFSET: usize, Owner, T>(&mut self) -> &mut Self
    where
        S: StorageTarget + Send + Sync + 'static,
        <T as FromStr>::Err: Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
        T: FromStr + Send + Sync + 'static,
    {
        self.add_parseable_field::<FIELD_OFFSET, Owner, T>();
        self
    }

    /// Adds an attribute type that implements FromStr,
    ///
    pub fn add_parseable_field<const FIELD_OFFSET: usize, Owner, T>(&mut self)
    where
        S: StorageTarget + Send + Sync + 'static,
        <T as FromStr>::Err: Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
        T: FromStr + Send + Sync + 'static,
    {
        self.add_type(AttributeTypeParser::parseable_field::<FIELD_OFFSET, Owner, T>());
    }

    /// Returns attribute parser with a parseable type, registered to ident, chainable
    ///
    pub fn with_parseable_as<const FIELD_OFFSET: usize, Owner, T>(
        &mut self,
        ident: impl Into<String>,
    ) -> &mut Self
    where
        S: StorageTarget + Send + Sync + 'static,
        <T as FromStr>::Err: Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
        T: FromStr + Send + Sync + 'static,
    {
        self.add_parseable_with::<FIELD_OFFSET, Owner, T>(ident.into());
        self
    }

    /// Adds an attribute type that implements FromStr w/ ident
    ///
    pub fn add_parseable_with<const FIELD_OFFSET: usize, Owner, T>(
        &mut self,
        ident: impl Into<String>,
    ) where
        S: StorageTarget + Send + Sync + 'static,
        <T as FromStr>::Err: Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
        T: FromStr + Send + Sync + 'static,
    {
        self.add_type_with(ident.into(), ParsableField::<FIELD_OFFSET, Owner, T>::parse);
    }

    /// Adds an attribute type that implements FromStr,
    ///
    pub fn add_parseable_attribute_type_field<const FIELD_OFFSET: usize, Owner, T>(&mut self)
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
        T: AttributeType<S> + Send + Sync + 'static,
    {
        self.add_type(AttributeTypeParser::parseable_attribute_type_field::<
            FIELD_OFFSET,
            Owner,
            T,
        >());
    }

    /// Returns attribute parser with a parseable type, registered to ident, chainable
    ///
    pub fn with_parseable_attribute_type_field_as<const FIELD_OFFSET: usize, Owner, T>(
        &mut self,
        ident: impl Into<String>,
    ) -> &mut Self
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
        T: AttributeType<S> + Send + Sync + 'static,
    {
        self.add_parseable_attribute_type_field_with::<FIELD_OFFSET, Owner, T>(ident.into());
        self
    }

    /// Adds an attribute type that implements FromStr w/ ident
    ///
    pub fn add_parseable_attribute_type_field_with<const FIELD_OFFSET: usize, Owner, T>(
        &mut self,
        ident: impl Into<String>,
    ) where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
        T: AttributeType<S> + Send + Sync + 'static,
    {
        self.add_type_with(
            ident.into(),
            ParsableAttributeTypeField::<FIELD_OFFSET, S, Owner, T>::parse,
        );
    }

    /// Sets the current tag value,
    ///
    pub fn set_tag(&mut self, tag: impl AsRef<str>) {
        self.tag = Some(tag.as_ref().to_string());
    }

    /// Sets the current name value,
    ///
    pub fn set_name(&mut self, name: impl AsRef<str>) {
        self.name = Some(name.as_ref().to_string());
    }

    /// Sets the current storage,
    ///
    pub fn set_storage(&mut self, storage: S) {
        self.storage = Some(Arc::new(RwLock::new(storage)));
    }

    /// Returns the current tag,
    ///
    pub fn tag(&self) -> Option<&String> {
        self.tag.as_ref()
    }

    /// Returns the current name,
    ///
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    /// Resets the parser state,
    ///
    pub fn reset(&mut self) {
        Option::take(&mut self.tag);
        Option::take(&mut self.name);
    }

    /// Returns an immutable reference to centralized-storage,
    ///
    pub fn storage(&self) -> Option<std::sync::RwLockReadGuard<S>> {
        if let Some(storage) = self.storage.as_ref() {
            storage.read().ok()
        } else {
            None
        }
    }

    /// Returns a mutable reference to centralized storage,
    ///
    pub fn storage_mut(&mut self) -> Option<std::sync::RwLockWriteGuard<S>> {
        if let Some(storage) = self.storage.as_ref() {
            storage.write().ok()
        } else {
            None
        }
    }
}

#[runmd::prelude::async_trait]
impl<S> ExtensionLoader for super::AttributeParser<S>
where
    S: StorageTarget + StorageTarget + Send + Sync + Unpin + 'static,
{
    async fn load_extension(&self, _extension: &str, _input: Option<&str>) -> Option<BoxedNode> {
        let mut parser = self.clone();

        if let Some(mut storage) = parser.storage_mut() {
            storage.drain_dispatch_queues();
        }

        Some(Box::pin(parser))
    }
}

impl<S> Node for super::AttributeParser<S>
where
    S: StorageTarget + StorageTarget + Send + Sync + Unpin + 'static,
{
    fn set_info(&mut self, _node_info: NodeInfo, _block_info: BlockInfo) {

    }

    fn define_property(&mut self, name: &str, tag: Option<&str>, input: Option<&str>) {
        self.reset();

        if let Some(tag) = tag.as_ref() {
            self.set_tag(tag);
            self.set_name(name);
        } else {
            self.set_name(name);
        }

        match self.attribute_types.get(name).cloned() {
            Some(cattr) => {
                cattr.parse(self, input.unwrap_or_default());
            }
            None => {
                trace!(attr_ty = name, "Did not have attribute");
            }
        }
    }

    fn completed(mut self: Box<Self>) {
        if let Some(mut storage) = self.storage_mut() {
            storage.drain_dispatch_queues();
        }
    }
}

// #[tokio::test]
// async fn test_v2_parser() {
//     let parser = AttributeParser::<World>::default();

//     let mut parser = parser
//         .load_extension("application/repo.reality.attributes.v1", None)
//         .await
//         .expect("should return a node");
//     parser.define_property("int", Some("test"), Some("256"));
//     parser.define_property("float", Some("test-2"), Some("256.0"));

//     println!("{:#?}", parser);
// }
