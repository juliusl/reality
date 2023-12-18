use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;
use tracing::debug;
use tracing::trace;

use runmd::prelude::*;

use super::attribute;
use super::attribute::Attribute;
use super::attribute::Property;
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
use crate::CallAsync;
use crate::Decoration;
use crate::FieldPacket;
use crate::KvpExt;
use crate::ResourceKey;
use crate::SetIdentifiers;
use crate::ThunkContext;

/// Represents a resource that has been assigned a path,
///
#[derive(Clone, Default)]
pub struct HostedResource {
    /// Address to list the resource under,
    ///
    pub address: String,
    /// The node resource key,
    ///
    pub node_rk: ResourceKey<crate::attributes::Node>,
    /// The hosted resource key,
    ///
    pub rk: ResourceKey<Attribute>,
    /// Any decorations to add w/ this resource,
    ///
    pub decoration: Option<Decoration>,
    /// Thunk context that is configured to the resource being hosted,
    ///
    pub binding: Option<ThunkContext>,
}

impl AsRef<ThunkContext> for HostedResource {
    fn as_ref(&self) -> &ThunkContext {
        self.binding.as_ref().expect("should be bound to a context")
    }
}

#[async_trait]
impl CallAsync for HostedResource {
    async fn call(tc: &mut ThunkContext) -> anyhow::Result<()> {
        tc.call().await?;
        Ok(())
    }
}

impl Debug for HostedResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedResource")
            .field("address", &self.address)
            .field("node_rk", &self.node_rk)
            .field("rk", &self.rk)
            .field("decoration", &self.decoration)
            .finish()
    }
}

impl SetIdentifiers for HostedResource {
    fn set_identifiers(&mut self, _: &str, _: Option<&String>) {}
}

/// Struct for a parsed block,
///
#[derive(Debug, Clone, Default)]
pub struct ParsedBlock {
    /// ParsedAttributes for each node,
    ///
    pub nodes: HashMap<ResourceKey<crate::attributes::Node>, ParsedAttributes>,
    /// Bounded parsed paths,
    ///
    pub paths: BTreeMap<String, ResourceKey<Attribute>>,
    /// Bounded resource paths,
    ///
    pub resource_paths: BTreeMap<String, HostedResource>,
}

impl ParsedBlock {
    /// Binds a node to a path in this parsed block,
    ///
    /// **Note** This will also update the node and bind the root path
    /// to the node's resource key.
    ///
    pub fn bind_node_to_path(
        &mut self,
        node: ResourceKey<attribute::Node>,
        path: impl Into<String>,
    ) -> bool {
        if let Some(_node) = self.nodes.get_mut(&node) {
            self.paths.insert(path.into(), node.transmute());
            true
        } else {
            false
        }
    }

    /// Gets the parsed attributes of a node,
    ///
    pub fn get_node(
        &self,
        node: ResourceKey<crate::attributes::Node>,
    ) -> Option<&ParsedAttributes> {
        self.nodes.get(&node)
    }

    /// Binds a path to a resource from a resource w/in the parsed block,
    ///
    pub fn bind_resource_path(
        &mut self,
        path: impl Into<String>,
        node: ResourceKey<crate::attributes::Node>,
        resource: ResourceKey<Attribute>,
        decoration: Option<Decoration>,
    ) -> &mut HostedResource {
        let path = path.into();

        self.resource_paths
            .entry(path.clone())
            .or_insert(HostedResource {
                address: path,
                node_rk: node,
                rk: resource,
                decoration,
                binding: None,
            })
    }

    /// Looks for a hosted resource by path,
    ///
    pub fn find_resource(&self, resource: impl AsRef<str>) -> Option<&HostedResource> {
        self.resource_paths.get(resource.as_ref())
    }

    /// Looks for a hosted resource by path and returns a mutable reference,
    ///
    pub fn find_resource_mut(&mut self, resource: impl AsRef<str>) -> Option<&mut HostedResource> {
        self.resource_paths.get_mut(resource.as_ref())
    }
}

/// Struct for parsed attributes,
///
#[derive(Debug, Default, Clone)]
pub struct ParsedAttributes {
    /// Node resource key,
    ///
    pub node: ResourceKey<Attribute>,
    /// Parsed attributes,
    ///
    pub attributes: Vec<ResourceKey<Attribute>>,
    /// Paths to attributes,
    ///
    pub paths: BTreeMap<String, ResourceKey<Attribute>>,
    /// Properties defined by parsed attributes,
    ///
    pub properties: Properties,
    /// Parsed lines,
    ///
    pub lines: Vec<String>,
    /// Map of labeled comments,
    ///
    pub comment_properties: HashMap<ResourceKey<Attribute>, BTreeMap<String, String>>,
    ///  Map of doc headers found for attributes,
    ///
    pub doc_headers: HashMap<ResourceKey<Attribute>, Vec<String>>,
}

/// Defined properties,
///
#[derive(Debug, Default, Clone)]
pub struct Properties {
    /// Map of defined properties,
    ///
    pub defined: HashMap<ResourceKey<Attribute>, Vec<ResourceKey<Property>>>,
    /// Map of labeled comments,
    ///
    pub comment_properties: HashMap<ResourceKey<Property>, BTreeMap<String, String>>,
    /// Map of doc headers found for properties,
    ///
    pub doc_headers: HashMap<ResourceKey<Property>, Vec<String>>,
}

impl ParsedAttributes {
    /// Returns the number of attributes that have been parsed,
    ///
    #[inline]
    pub fn len(&self) -> usize {
        self.attributes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Push a new parsed attribute,
    ///
    #[inline]
    pub fn push(&mut self, attr: ResourceKey<Attribute>) {
        self.attributes.push(attr);
    }

    /// Returns the last attrbute key parsed,
    ///
    #[inline]
    pub fn last(&self) -> Option<&ResourceKey<Attribute>> {
        self.attributes.last()
    }

    /// Bind a path to the last attribute,
    ///
    #[inline]
    pub fn bind_last_to_path(&mut self, path: String) {
        if let Some(last) = self.attributes.last() {
            self.paths.insert(path, *last);
        } else {
            self.paths.insert(path, self.node);
        }
    }

    /// Returns an iterator over parsed attributes,
    ///
    #[inline]
    pub fn parsed(&self) -> impl Iterator<Item = ResourceKey<Attribute>> + '_ {
        self.attributes.iter().cloned()
    }

    /// Resolve a path,
    ///
    #[inline]
    pub fn resolve_path(&self, path: impl AsRef<str>) -> Option<&ResourceKey<Attribute>> {
        self.paths.get(path.as_ref())
    }

    /// Defines a property by attr,
    ///
    #[inline]
    pub fn define_property(&mut self, attr: ResourceKey<Attribute>, prop: ResourceKey<Property>) {
        let defined = self.properties.defined.entry(attr).or_default();
        defined.push(prop);
    }

    /// Adds a comment to the last property,
    ///
    #[inline]
    pub fn add_line(&mut self, line: &Line) {
        for line in line.to_string().lines() {
            self.lines.push(line.to_string());
        }

        if !line.comment_properties.is_empty() {
            if line.attr.is_some() && line.extension.is_some() {
                if let Some(last) = self.last() {
                    if let Some(last) = self.properties.defined.get(last).and_then(|l| l.last()) {
                        self.properties
                            .comment_properties
                            .insert(*last, line.comment_properties.clone());
                    } else {
                        eprintln!("a -- skipped {:?}", line)
                    }
                } else {
                    eprintln!("b -- skipped {:?}", line)
                }
            } else if line.extension.is_some() && line.attr.is_none() {
                if let Some(last) = self.last().cloned() {
                    self.comment_properties
                        .insert(last, line.comment_properties.clone());
                } else if line.extension.is_none() && line.attr.is_some() {
                    if let Some(last) = self.last().cloned() {
                        self.comment_properties
                            .insert(last, line.comment_properties.clone());
                    } else {
                        eprintln!("c -- skipped {:?}", line)
                    }
                } else {
                    eprintln!("d -- skipped {:?}", line)
                }
            } else if line.extension.is_none() && line.attr.is_some() {
                if let Some(last) = self.last() {
                    if let Some(last) = self.properties.defined.get(last).and_then(|l| l.last()) {
                        self.properties
                            .comment_properties
                            .insert(*last, line.comment_properties.clone());
                    } else {
                        self.comment_properties
                            .insert(self.node, line.comment_properties.clone());
                    }
                } else if line.extension.is_none() && line.attr.is_some() {
                    if let Some(last) = self.last().cloned() {
                        self.comment_properties
                            .insert(last, line.comment_properties.clone());
                    } else {
                        // eprintln!("f -- skipped {:?}", line);
                        // eprintln!("{:?}", self);
                        self.comment_properties
                            .insert(self.node, line.comment_properties.clone());
                    }
                }
            } else {
                eprintln!("g -- skipped {:?}", line)
            }
        }

        if !line.doc_headers.is_empty() {
            if line.attr.is_some() && line.extension.is_some() {
                if let Some(last) = self.last() {
                    if let Some(last) = self.properties.defined.get(last).and_then(|l| l.last()) {
                        self.properties.doc_headers.insert(
                            *last,
                            line.doc_headers.iter().map(|h| h.to_string()).collect(),
                        );
                    } else {
                        eprintln!("h -- skipped {:?}", line)
                    }
                } else {
                    eprintln!("i -- skipped {:?}", line)
                }
            } else if line.extension.is_some() && line.attr.is_none() {
                if let Some(last) = self.last().cloned() {
                    self.doc_headers.insert(
                        last,
                        line.doc_headers.iter().map(|h| h.to_string()).collect(),
                    );
                } else if line.extension.is_none() && line.attr.is_some() {
                    if let Some(last) = self.last().cloned() {
                        self.doc_headers.insert(
                            last,
                            line.doc_headers.iter().map(|h| h.to_string()).collect(),
                        );
                    } else {
                        eprintln!("j -- skipped {:?}", line)
                    }
                } else {
                    //  eprintln!("l -- skipped {:?}", line)
                    self.doc_headers.insert(
                        self.node,
                        line.doc_headers.iter().map(|h| h.to_string()).collect(),
                    );
                }
            } else if line.extension.is_none() && line.attr.is_some() {
                if let Some(last) = self.last() {
                    if let Some(last) = self.properties.defined.get(last).and_then(|l| l.last()) {
                        self.properties.doc_headers.insert(
                            *last,
                            line.doc_headers.iter().map(|h| h.to_string()).collect(),
                        );
                    } else {
                        // eprintln!("m -- skipped {:?}", line)
                        self.doc_headers.insert(
                            self.node,
                            line.doc_headers.iter().map(|h| h.to_string()).collect(),
                        );
                    }
                } else if line.extension.is_none() && line.attr.is_some() {
                    if let Some(last) = self.last().cloned() {
                        self.doc_headers.insert(
                            last,
                            line.doc_headers.iter().map(|h| h.to_string()).collect(),
                        );
                    } else {
                        // eprintln!("n -- skipped {:?}", line);
                        // eprintln!("{:?}", self);
                        self.doc_headers.insert(
                            self.node,
                            line.doc_headers.iter().map(|h| h.to_string()).collect(),
                        );
                    }
                } else {
                    eprintln!("o -- skipped {:?}", line)
                }
            } else {
                eprintln!("p -- skipped {:?}", line)
            }
        }
    }

    /// Finds any parsed extras for a resource key,
    ///
    #[inline]
    pub async fn index_decorations(&self, rk: ResourceKey<Attribute>, tc: &mut ThunkContext) {
        tc.store_kv(
            rk,
            Decoration {
                comment_properties: self.comment_properties.get(&rk).cloned(),
                doc_headers: self.doc_headers.get(&rk).cloned(),
            },
        );

        let mut props = vec![];
        if let Some(properties) = self.properties.defined.get(&rk) {
            for prop in properties {
                tc.store_kv(
                    prop,
                    Decoration {
                        comment_properties: self.properties.comment_properties.get(prop).cloned(),
                        doc_headers: self.properties.doc_headers.get(prop).cloned(),
                    },
                );
                props.push(prop);
            }
        }

        let mut packets = vec![];
        {
            let node = tc.node().await;
            for prop in props {
                if let Some(fp) = node.current_resource::<FieldPacket>(prop.transmute()) {
                    packets.push((*prop, fp));
                }
            }
        }

        for (pk, fp) in packets {
            tc.store_kv(pk, fp);
        }
    }
}

/// List of comments for an attribute,
///
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Comments(pub Vec<String>);

/// Struct containing a tag value,
///
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tag(String);

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
    pub attributes: ParsedAttributes,
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
            attributes: ParsedAttributes::default(),
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
            .field("attributes", &self.attributes)
            .finish()
    }
}

impl<S: StorageTarget> AttributeParser<S> {
    /// Parses an attribute and if successful returns the resource key used,
    ///
    pub fn parse_attribute<T: FromStr + Send + Sync + 'static>(
        &mut self,
        source: impl AsRef<str>,
    ) -> anyhow::Result<ResourceKey<T>> {
        let idx = self.attributes.len();
        let key = ResourceKey::<Attribute>::with_hash(idx);

        // Storage target must be enabled,
        if let Some(storage) = self.storage() {
            // Initialize attribute type,
            let init = source.as_ref().parse::<T>().map_err(|_| {
                anyhow::anyhow!(
                    "Could not parse {} from {}",
                    std::any::type_name::<T>(),
                    source.as_ref()
                )
            })?;
            storage.lazy_put_resource(init, key.transmute());
            if let Some(tag) = self.tag() {
                storage.lazy_put_resource(Tag(tag.to_string()), key.transmute());
            }
        }
        self.attributes.push(key);

        Ok(key.transmute())
    }

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
        debug!("Enabling object type {}", object_ty.ident);
        self.block_object_types
            .insert(object_ty.ident.to_string(), object_ty);
    }

    pub fn add_object_type_with(&mut self, ident: &str, object_ty: impl Into<BlockObjectType<S>>) {
        let object_ty = object_ty.into();
        debug!("Enabling object type {}", object_ty.ident);
        self.block_object_types.insert(ident.to_string(), object_ty);
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
    pub fn with_parseable_field<const FIELD_OFFSET: usize, Owner>(&mut self) -> &mut Self
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        <Owner::ParseType as FromStr>::Err: Send + Sync + 'static,
    {
        self.add_parseable_field::<FIELD_OFFSET, Owner>();
        self
    }

    /// Adds an attribute type that implements FromStr,
    ///
    pub fn add_parseable_field<const FIELD_OFFSET: usize, Owner>(&mut self)
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        <Owner::ParseType as FromStr>::Err: Send + Sync + 'static,
    {
        self.add_type(AttributeTypeParser::parseable_field::<FIELD_OFFSET, Owner>());
    }

    /// Returns attribute parser with a parseable type, registered to ident, chainable
    ///
    pub fn with_parseable_as<const FIELD_OFFSET: usize, Owner>(
        &mut self,
        ident: impl Into<String>,
    ) -> &mut Self
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        <Owner::ParseType as FromStr>::Err: Send + Sync + 'static,
    {
        self.add_parseable_with::<FIELD_OFFSET, Owner>(ident.into());
        self
    }

    /// Adds an attribute type that implements FromStr w/ ident
    ///
    pub fn add_parseable_with<const FIELD_OFFSET: usize, Owner>(&mut self, ident: impl Into<String>)
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        <Owner::ParseType as FromStr>::Err: Send + Sync + 'static,
    {
        self.add_type_with(ident.into(), ParsableField::<FIELD_OFFSET, Owner>::parse);
    }

    /// Adds an attribute type that implements FromStr,
    ///
    pub fn add_parseable_attribute_type_field<const FIELD_OFFSET: usize, Owner>(&mut self)
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        Owner::ParseType: AttributeType<S> + Send + Sync + 'static,
    {
        self.add_type(AttributeTypeParser::parseable_attribute_type_field::<
            FIELD_OFFSET,
            Owner,
        >());
    }

    /// Adds an attribute type that implements FromStr,
    ///
    pub fn add_parseable_extension_type_field<const FIELD_OFFSET: usize, Owner>(&mut self)
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        Owner::ParseType: BlockObject<S> + Send + Sync + 'static,
    {
        self.add_type(AttributeTypeParser::parseable_object_type_field::<
            FIELD_OFFSET,
            Owner,
        >());
    }

    /// Returns attribute parser with a parseable type, registered to ident, chainable
    ///
    pub fn with_parseable_attribute_type_field_as<const FIELD_OFFSET: usize, Owner>(
        &mut self,
        ident: impl Into<String>,
    ) -> &mut Self
    where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        Owner::ParseType: AttributeType<S> + Send + Sync + 'static,
    {
        self.add_parseable_attribute_type_field_with::<FIELD_OFFSET, Owner>(ident.into());
        self
    }

    /// Adds an attribute type that implements FromStr w/ ident
    ///
    pub fn add_parseable_attribute_type_field_with<const FIELD_OFFSET: usize, Owner>(
        &mut self,
        ident: impl Into<String>,
    ) where
        S: StorageTarget + Send + Sync + 'static,
        Owner: OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        Owner::ParseType: AttributeType<S> + Send + Sync + 'static,
    {
        self.add_type_with(
            ident.into(),
            ParsableAttributeTypeField::<FIELD_OFFSET, S, Owner>::parse,
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
    fn assign_path(&mut self, path: String) {
        self.attributes.bind_last_to_path(path);
    }

    fn set_info(&mut self, _node_info: NodeInfo, _block_info: BlockInfo) {
        trace!("{:#?}", _node_info);
        if _node_info.parent_idx.is_none() {
            let last = self.attributes.attributes.last();
            trace!("Add node {:?} {:?}", _node_info, last);
        }
    }

    fn parsed_line(&mut self, _node_info: NodeInfo, _block_info: BlockInfo) {
        trace!("[PARSED] {}", _node_info.line);
        self.attributes.add_line(&_node_info.line);
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
        if let Some(storage) = self.storage() {
            storage.lazy_put_resource(self.attributes.clone(), ResourceKey::root());
        }

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
    async fn load_extension(
        &self,
        extension: &str,
        tag: Option<&str>,
        input: Option<&str>,
    ) -> Option<BoxedNode> {
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

        if let Some(tag) = tag {
            parser.set_tag(tag);
        }

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

            // Extension has been loaded to a namespace
            handler
                .on_load(namespace, parser.attributes.last().copied())
                .await;
            parser.handlers.push(handler);

            // Drain any dispatches before trying to load the rest of the resources
            if let Some(mut storage) = parser.storage_mut() {
                storage.drain_dispatch_queues();
            }
        }

        Some(Box::pin(parser))
    }
}
