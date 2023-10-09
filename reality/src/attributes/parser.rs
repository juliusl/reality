use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;
use tracing::trace;

use runmd::prelude::*;

use super::attribute::Attribute;
use super::attribute_type::OnParseField;
use super::attribute_type::ParsableAttributeTypeField;
use super::attribute_type::ParsableField;
use super::AttributeTypeParser;
use super::StorageTarget;
use crate::block::BlockObjectHandler;
use crate::AsyncStorageTarget;
use crate::AttributeType;
use crate::BlockObject;
use crate::BlockObjectType;
use crate::ResourceKey;

/// Maintains attribute types and matches runmd nodes to the corresponding attribute type parser,
///
pub struct AttributeParser<Storage: StorageTarget + 'static> {
    /// Current name being parsed,
    ///
    name: Option<String>,
    /// Current tag being parsed,
    ///
    tag: Option<String>,
    /// Block object types,
    ///
    block_object_types: BTreeMap<String, BlockObjectType<Storage>>,
    /// Table of attribute type parsers,
    ///
    attribute_types: BTreeMap<String, AttributeTypeParser<Storage>>,
    /// Stack of block object handlers to call on specific events,
    ///
    handlers: Vec<BlockObjectHandler<Storage::Namespace>>,
    /// Reference to centralized-storage,
    ///
    storage: Option<Arc<tokio::sync::RwLock<Storage>>>,
    /// Attributes parsed,
    /// 
    attributes: Vec<ResourceKey<Attribute>>,
}

impl<S: StorageTarget + 'static> Default for AttributeParser<S> {
    fn default() -> Self {
        Self {
            name: Default::default(),
            tag: Default::default(),
            block_object_types: Default::default(),
            attribute_types: Default::default(),
            handlers: Default::default(),
            storage: Default::default(),
            attributes: vec![],
        }
    }
}

impl<S: StorageTarget + 'static> Clone for AttributeParser<S> {
    fn clone(&self) -> Self {
        Self {
            tag: self.tag.clone(),
            name: self.name.clone(),
            attribute_types: self.attribute_types.clone(),
            block_object_types: self.block_object_types.clone(),
            handlers: self.handlers.clone(),
            storage: self.storage.clone(),
            attributes: self.attributes.clone(),
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
    /// Adds an object type to the parser,
    ///
    pub fn with_object_type<O: BlockObject<S>>(&mut self) -> &mut Self {
        self.add_object_type(BlockObjectType::new::<O>());
        self
    }

    /// Adds an object type to the parser,
    ///
    pub fn add_object_type(&mut self, object_ty: impl Into<BlockObjectType<S>>) {
        let object_ty = object_ty.into();

        self.block_object_types
            .insert(object_ty.ident.to_string(), object_ty);
    }

    /// Adds an attribute type to the parser and returns self,
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
    pub fn add_type(&mut self, attr_ty: impl Into<AttributeTypeParser<S>>) {
        let custom_attr = attr_ty.into();
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
    pub fn set_storage(&mut self, storage: Arc<tokio::sync::RwLock<S>>) {
        self.storage = Some(storage);
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

    /// Returns a clone of storage,
    ///
    pub fn clone_storage(&self) -> Option<Arc<tokio::sync::RwLock<S>>> {
        self.storage.clone()
    }

    /// Returns an immutable reference to centralized-storage,
    ///
    pub fn storage<'a: 'b, 'b>(&'a self) -> Option<tokio::sync::RwLockReadGuard<'b, S>> {
        if let Some(storage) = self.storage.as_ref() {
            storage.try_read().ok()
        } else {
            None
        }
    }

    /// Returns a mutable reference to centralized storage,
    ///
    pub fn storage_mut<'a: 'b, 'b>(&'a mut self) -> Option<tokio::sync::RwLockWriteGuard<'b, S>> {
        if let Some(storage) = self.storage.as_ref() {
            storage.try_write().ok()
        } else {
            None
        }
    }

    /// Returns a shared namespace or creates one if one doesn't exist,
    ///
    /// Returns None if namespaces are disabled by the storage target.
    ///
    #[cfg(feature = "async_dispatcher")]
    pub fn namespace(
        &mut self,
        namespace: impl Into<String>,
    ) -> Option<AsyncStorageTarget<S::Namespace>>
    where
        S: 'static,
    {
        let namespace = namespace.into();

        // Check if an async_target already exists,
        //
        let async_target =
            resource_owned!(self, AsyncStorageTarget<S::Namespace>, namespace.clone());
        if async_target.is_some() {
            return async_target;
        }

        // Otherwise, create a new namespace if enabled and add as a resource
        //
        if let Some(mut storage) = self.storage_mut() {
            let ns = storage.shared_namespace(namespace.clone());
            storage.drain_dispatch_queues();
            Some(ns)
        } else {
            None
        }
    }
}

impl<S> Node for super::AttributeParser<S>
where
    S: StorageTarget + StorageTarget + Send + Sync + Unpin + 'static,
{
    fn set_info(&mut self, _node_info: NodeInfo, _block_info: BlockInfo) {
        trace!("{:#?}", _node_info);

        let _resource_key = ResourceKey::<()>::with_hash(_node_info);
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

        for handler in self.handlers {
            handler.on_completed();
        }
    }
}

#[runmd::prelude::async_trait]
impl<S> ExtensionLoader for super::AttributeParser<S>
where
    S: StorageTarget + Send + Sync + Unpin + 'static,
    <S as StorageTarget>::Namespace: Send + Sync + 'static,
{
    async fn load_extension(&self, extension: &str, input: Option<&str>) -> Option<BoxedNode> {
        let mut parser = self.clone();

        // If there was a handler on the stack, call it's unload fn
        if let Some(handler) = parser.handlers.last() {
            handler.on_unload().await;

            // Drain any dispatches before trying to load the rest of the resources
            if let Some(mut storage) = parser.storage_mut() {
                storage.drain_dispatch_queues();
            }
        }

        // Clear any pre-existing attribute types
        parser.attribute_types.clear();

        // If an block object-type exists, then begin to load
        if let (
            Some(BlockObjectType {
                attribute_type,
                mut handler,
                ..
            }),
            Some(namespace),
        ) = (
            parser.block_object_types.get(extension).cloned(),
            parser.namespace(extension),
        ) {
            attribute_type.parse(&mut parser, input.unwrap_or_default());

            // Drain any dispatches before trying to load the rest of the resources
            if let Some(mut storage) = parser.storage_mut() {
                storage.drain_dispatch_queues();
            }

            handler.on_load(namespace).await;
            parser.handlers.push(handler);

            // Drain any dispatches before trying to load the rest of the resources
            if let Some(mut storage) = parser.storage_mut() {
                storage.drain_dispatch_queues();
            }
        }

        Some(Box::pin(parser))
    }
}
