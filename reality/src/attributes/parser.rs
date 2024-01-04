use anyhow::anyhow;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tracing::debug;
use tracing::error;
use tracing::trace;

use runir::prelude::*;
use runmd::prelude::*;

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
use crate::LinkFieldFn;
use crate::LinkRecvFn;
use crate::PluginLevel;
use crate::ResourceKey;
use crate::SetIdentifiers;
use crate::Shared;
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
            .finish()
    }
}

impl SetIdentifiers for HostedResource {
    fn set_identifiers(&mut self, _: &str, _: Option<&String>) {}
}

/// Struct for parsed attributes,
///
#[derive(Debug, Default, Clone)]
pub struct ParsedNode {
    /// Node resource key,
    ///
    pub node: ResourceKey<Attribute>,
    /// Parsed attributes,
    ///
    pub attributes: Vec<ResourceKey<Attribute>>,
    /// Paths to attributes,
    ///
    pub paths: BTreeMap<String, ResourceKey<Attribute>>,
    /// Defined properties,
    ///
    pub properties: Vec<ResourceKey<Property>>,
}

impl ParsedNode {
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
        self.attributes
            .iter()
            .cloned()
            .chain(self.properties.iter().map(|p| p.transmute()))
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
    pub fn define_property(&mut self, prop: ResourceKey<Property>) {
        self.properties.push(prop);
    }

    /// Parses and finalizes the current node state,
    ///
    pub(crate) async fn parse<I: InternerFactory>(
        &mut self,
        interner: impl Fn() -> I,
        storage: &Shared,
    ) -> anyhow::Result<()> {
        if let Some(mut repr) = self.node.repr() {
            if let Some(node) = repr.as_node() {
                let input = node.input().map(|s| s.to_string()).unwrap_or(String::new());
                let tag = node.tag();

                let address = if let Some(ref tag) = tag {
                    format!("{input}#{tag}")
                } else {
                    input.to_string()
                };

                let mut exts = self
                    .attributes
                    .iter()
                    .filter_map(|a| a.repr())
                    .collect::<Vec<_>>();

                for (i, e) in exts.iter_mut().enumerate() {
                    if let Some(node) = e.as_node() {
                        if let Some(path) = node.path() {
                            let address = if let Some(ref tag) = tag {
                                format!("{input}/{}#{tag}", path)
                            } else {
                                format!("{input}/{}", path)
                            };

                            let host = HostLevel::new(address);
                            e.upgrade(interner(), host).await?;

                            if let Some(ext) = self.attributes.get(i) {
                                if let Some(plugin) =
                                    storage.resource::<PluginLevel>(ext.transmute())
                                {
                                    trace!("Upgrading ext w/ plugin");
                                    e.upgrade(interner(), plugin.clone()).await?;
                                }
                            }
                        }
                    }
                }

                let mut host = HostLevel::new(address.clone());
                host.set_extensions(exts);

                trace!("Upgrading node w/ host -- {}", address);

                repr.upgrade(interner(), host).await?;

                if let Some(plugin) = storage.resource::<PluginLevel>(self.node.transmute()) {
                    trace!("Upgrading node w/ plugin");
                    repr.upgrade(interner(), plugin.clone()).await?;
                }

                self.node.set_repr(repr);

                for (f, a) in self.attributes.iter_mut().map(|a| (a.clone().repr(), a)) {
                    if let Some(mut f) = f {
                        let node = f.as_node();
                        if let Some(path) = node.as_ref().and_then(NodeRepr::path) {
                            let address = if let Some(ref tag) = tag {
                                format!("{input}/{path}#{tag}")
                            } else {
                                format!("{input}/{path}")
                            };

                            trace!("Upgrading field w/ host -- {}", address);
                            f.upgrade(interner(), HostLevel::new(address)).await?;

                            if let Some(plugin) = storage.resource::<PluginLevel>(a.transmute()) {
                                trace!("Upgrading field w/ plugin");
                                f.upgrade(interner(), plugin.clone()).await?;
                            }

                            a.set_repr(f);
                        }
                    }
                }
            }
        } else {
            Err(anyhow!("Node is empty"))?;
        }

        for (_, rk) in self.paths.iter_mut() {
            if self.node.key() == rk.key() {
                if let Some(repr) = self.node.repr() {
                    rk.set_repr(repr);
                }
            }
            if let Some(attr) = self.attributes.iter().find(|r| r.key() == rk.key()) {
                if let Some(repr) = attr.repr() {
                    rk.set_repr(repr);
                }
            }
        }

        Ok(())
    }
}

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
    block_object_types: BTreeMap<String, BlockObjectType>,
    /// Table of attribute type parsers,
    ///
    attribute_types: BTreeMap<String, AttributeTypeParser<Storage>>,
    /// Stack of block object handlers to call on specific events,
    ///
    handlers: Vec<BlockObjectHandler>,
    /// Reference to centralized-storage,
    ///
    storage: Option<Arc<tokio::sync::RwLock<Storage>>>,
    /// Sets the relative path of the source,
    ///
    pub(crate) relative: Option<PathBuf>,
    /// Attributes parsed,
    ///
    pub parsed_node: ParsedNode,
    /// Stack of nodes parsed from source,
    ///
    pub(crate) nodes: Vec<NodeLevel>,
    /// Stack of fields that have been parsed,
    ///
    pub(crate) fields: Vec<Repr>,
    /// Stack of link recv fns,
    ///
    pub(crate) link_recv: Vec<LinkRecvFn>,
}

impl<S: StorageTarget + 'static> Default for AttributeParser<S> {
    fn default() -> Self {
        Self {
            name: Default::default(),
            tag: Default::default(),
            relative: None,
            block_object_types: Default::default(),
            attribute_types: Default::default(),
            handlers: Default::default(),
            storage: Default::default(),
            parsed_node: ParsedNode::default(),
            nodes: vec![],
            fields: vec![],
            link_recv: vec![],
        }
    }
}

impl<S: StorageTarget + 'static> Clone for AttributeParser<S> {
    fn clone(&self) -> Self {
        Self {
            tag: self.tag.clone(),
            name: self.name.clone(),
            relative: self.relative.clone(),
            attribute_types: self.attribute_types.clone(),
            block_object_types: self.block_object_types.clone(),
            handlers: self.handlers.clone(),
            storage: self.storage.clone(),
            parsed_node: self.parsed_node.clone(),
            nodes: self.nodes.clone(),
            fields: self.fields.clone(),
            link_recv: self.link_recv.clone(),
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
            .field("attributes", &self.parsed_node)
            .field("fields", &self.fields)
            .finish()
    }
}

impl AttributeParser<Shared> {
    /// Parses an attribute and if successful returns the resource key used,
    ///
    pub fn parse_attribute<T: FromStr + Send + Sync + 'static>(
        &mut self,
        source: impl AsRef<str>,
    ) -> anyhow::Result<ResourceKey<T>> {
        let tag = self.tag().cloned();
        if let Some(last) = self.nodes.last_mut() {
            last.set_input(source.as_ref());

            if let Some(tag) = tag {
                last.set_tag(tag);
            }
        }

        trace!(
            "parsing attribute -- `{}` {:?}",
            source.as_ref(),
            self.nodes
                .last()
                .cloned()
                .unwrap_or(NodeLevel::new())
                .mount()
        );
        // let idx = self.parsed_node.len();
        let key = ResourceKey::<Attribute>::with_hash(
            self.nodes
                .last()
                .cloned()
                .unwrap_or(NodeLevel::new())
                .mount(),
        );

        // Storage target must be enabled,
        if let Some(mut storage) = self.storage_mut() {
            // Initialize attribute type,
            let init = source.as_ref().parse::<T>().map_err(|_| {
                anyhow::anyhow!(
                    "Could not parse {} from {}",
                    std::any::type_name::<T>(),
                    source.as_ref()
                )
            })?;
            storage.put_resource(init, key.transmute());
        }
        self.parsed_node.push(key);

        Ok(key.transmute())
    }

    /// Adds an object type to the parser,
    ///
    pub fn with_object_type<O: BlockObject>(&mut self) -> &mut Self {
        self.add_object_type(BlockObjectType::new::<O>());
        self
    }

    /// Adds an object type to the parser w/ a different inner,
    ///
    pub fn with_object_type_as<O: BlockObject, As: BlockObject>(&mut self) -> &mut Self {
        self.add_object_type(BlockObjectType::new_as::<O, As>());
        self
    }

    /// Pushes a link recv task,
    ///
    pub fn push_link_recv<T>(&mut self)
    where
        T: runir::prelude::Recv + Send + Sync + 'static,
    {
        self.link_recv.push(|n, f| {
            Box::pin(async move {
                let mut repr = Linker::new::<T>();
                repr.push_level(RecvLevel::new::<T>(f))?;
                repr.push_level(n)?;
                repr.link().await
            })
        })
    }

    /// Adds an object type to the parser,
    ///
    pub fn add_object_type(&mut self, object_ty: impl Into<BlockObjectType>) {
        let object_ty = object_ty.into();
        debug!("Enabling object type {}", object_ty.ident);
        self.block_object_types
            .insert(object_ty.ident.to_string(), object_ty);
    }

    pub fn add_object_type_with(&mut self, ident: &str, object_ty: impl Into<BlockObjectType>) {
        let object_ty = object_ty.into();
        debug!("Enabling object type {}", object_ty.ident);
        self.block_object_types.insert(ident.to_string(), object_ty);
    }

    /// Adds an attribute type to the parser and returns self,
    ///
    pub fn with_type<C>(&mut self) -> &mut Self
    where
        C: AttributeType<Shared> + Send + Sync + 'static,
    {
        self.add_type(AttributeTypeParser::new::<C>(ResourceLevel::new::<C>()));
        self
    }

    /// Adds a custom attribute parser,
    ///
    pub fn add_type(&mut self, attr_ty: impl Into<AttributeTypeParser<Shared>>) {
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
        parse: fn(&mut AttributeParser<Shared>, String),
        link_recv: LinkRecvFn,
        link_field: LinkFieldFn,
        resource: ResourceLevel,
        field: FieldLevel,
    ) -> AttributeTypeParser<Shared> {
        let attr = AttributeTypeParser::new_with(
            ident,
            parse,
            link_recv,
            link_field,
            resource,
            Some(field),
        );
        self.add_type(attr.clone());
        attr
    }

    /// Returns attribute parser with a parseable type, chainable
    ///
    pub fn with_parseable_field<const FIELD_OFFSET: usize, Owner>(&mut self) -> &mut Self
    where
        Owner: Recv + OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        <Owner::ParseType as FromStr>::Err: Send + Sync + 'static,
    {
        self.add_parseable_field::<FIELD_OFFSET, Owner>();
        self
    }

    /// Adds an attribute type that implements FromStr,
    ///
    pub fn add_parseable_field<const FIELD_OFFSET: usize, Owner>(&mut self)
    where
        Owner: Recv + OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
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
        Owner: Recv + OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        <Owner::ParseType as FromStr>::Err: Send + Sync + 'static,
    {
        self.add_parseable_with::<FIELD_OFFSET, Owner>(ident.into());
        self
    }

    /// Adds an attribute type that implements FromStr w/ ident
    ///
    pub fn add_parseable_with<const FIELD_OFFSET: usize, Owner>(&mut self, ident: impl Into<String>)
    where
        Owner: Recv + OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        <Owner::ParseType as FromStr>::Err: Send + Sync + 'static,
    {
        self.add_type_with(
            ident.into(),
            ParsableField::<FIELD_OFFSET, Owner>::parse,
            Owner::link_recv,
            Owner::link_field,
            ResourceLevel::new::<Owner::ProjectedType>(),
            FieldLevel::new::<FIELD_OFFSET, Owner>(),
        );
    }

    /// Adds an attribute type that implements FromStr,
    ///
    pub fn add_parseable_attribute_type_field<const FIELD_OFFSET: usize, Owner>(&mut self)
    where
        Owner: Recv + OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        Owner::ParseType: AttributeType<Shared> + Send + Sync + 'static,
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
        Owner: Recv + OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        Owner::ParseType: BlockObject + Send + Sync + 'static,
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
        Owner: Recv + OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        Owner::ParseType: AttributeType<Shared> + Send + Sync + 'static,
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
        Owner: Recv + OnParseField<FIELD_OFFSET> + Send + Sync + 'static,
        Owner::ParseType: AttributeType<Shared> + Send + Sync + 'static,
    {
        self.add_type_with(
            ident.into(),
            ParsableAttributeTypeField::<FIELD_OFFSET, Owner>::parse,
            Owner::link_recv,
            Owner::link_field,
            ResourceLevel::new::<Owner::ProjectedType>(),
            FieldLevel::new::<FIELD_OFFSET, Owner>(),
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
    pub fn set_storage(&mut self, storage: Arc<tokio::sync::RwLock<Shared>>) {
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
    pub fn clone_storage(&self) -> Option<Arc<tokio::sync::RwLock<Shared>>> {
        self.storage.clone()
    }

    /// Returns an immutable reference to centralized-storage,
    ///
    pub fn storage<'a: 'b, 'b>(&'a self) -> Option<tokio::sync::RwLockReadGuard<'b, Shared>> {
        if let Some(storage) = self.storage.as_ref() {
            storage.try_read().ok()
        } else {
            None
        }
    }

    /// Returns a mutable reference to centralized storage,
    ///
    pub fn storage_mut<'a: 'b, 'b>(
        &'a mut self,
    ) -> Option<tokio::sync::RwLockWriteGuard<'b, Shared>> {
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
    ) -> Option<AsyncStorageTarget<Shared>> {
        let namespace = namespace.into();

        trace!("Getting namespace {namespace}");

        // Check if an async_target already exists,
        //
        let async_target = resource_owned!(self, AsyncStorageTarget<Shared>, namespace.clone());
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

#[async_trait(?Send)]
impl Node for super::AttributeParser<Shared> {
    fn assign_path(&mut self, path: String) {
        if let Some(node) = self.nodes.last_mut() {
            trace!("Setting path -- {} -- {:?}", path.as_str(), node.mount());
            node.set_path(path.as_str());
        }

        self.parsed_node.bind_last_to_path(path);
    }

    fn set_info(&mut self, _node_info: NodeInfo, _block_info: BlockInfo) {
        trace!("{:#?}", _node_info);
        if _node_info.parent_idx.is_none() {
            let last = self.parsed_node.attributes.last();
            trace!("Add node {:?} {:?}", _node_info, last);
        } else {
            // When adding a node, the node level is set before this fn
            let node = NodeLevel::new()
                .with_doc_headers(_node_info.line.doc_headers)
                .with_annotations(_node_info.line.comment_properties)
                .with_idx(_node_info.idx)
                .with_block(_block_info.idx)
                .with_source_span(_node_info.span.unwrap_or_default())
                .with_source_relative(self.relative.clone().unwrap_or_default());
            self.nodes.push(node);
        }
    }

    fn parsed_line(&mut self, _node_info: NodeInfo, _block_info: BlockInfo) {
        trace!("[PARSED]\n\n{}\n", _node_info.line);
        if let Some(last) = self.nodes.last_mut() {
            last.set_source(_node_info.line.to_string());
        }
    }

    async fn define_property(&mut self, name: &str, tag: Option<&str>, input: Option<&str>) {
        self.reset();

        // Configure the current node
        if let Some(last) = self.nodes[..].last_mut() {
            last.set_symbol(name);
            if let Some(input) = input {
                last.set_input(input);
            }
            if let Some(tag) = tag {
                last.set_tag(tag);
            }
        }

        if let Some(tag) = tag.as_ref() {
            self.set_tag(tag);
            self.set_name(name);
        } else {
            self.set_name(name);
        }

        match self.attribute_types.get(name).cloned() {
            Some(cattr) => {
                cattr.parse(self, input.unwrap_or_default());

                if let Some(last) = self.nodes.last() {
                    match cattr.link_field(last.clone()).await {
                        Ok(field_repr) => {
                            self.fields.push(field_repr);

                            if let Some(last) = self.parsed_node.properties.last_mut() {
                                last.set_repr(field_repr);
                            }
                        }
                        Err(err) => {
                            error!("{err}");
                        }
                    }
                }
                self.link_recv.push(cattr.link_recv);
            }
            None => {
                trace!(attr_ty = name, "Did not have attribute");
            }
        }
    }

    fn completed(mut self: Box<Self>) {
        if let Some(storage) = self.storage() {
            storage.lazy_put_resource(self.parsed_node.clone(), ResourceKey::root());
        }

        if let Some(mut storage) = self.storage_mut() {
            storage.drain_dispatch_queues();
        }

        for handler in self.handlers {
            handler.on_completed();
        }
    }
}

#[runmd::prelude::async_trait(?Send)]
impl ExtensionLoader for super::AttributeParser<Shared> {
    async fn load_extension(
        &self,
        extension: &str,
        tag: Option<&str>,
        input: Option<&str>,
    ) -> Option<BoxedNode> {
        let mut parser = self.clone();

        if parser.fields.len() > 0 {
            let _ = parser.unload().await;
        }

        // Clear any pre-existing attribute types
        parser.attribute_types.clear();

        if let Some(tag) = tag {
            parser.set_tag(tag);
        }

        // // Configure node properties
        if let Some(node) = parser.nodes[..].last_mut() {
            node.set_symbol(extension);

            if let Some(input) = input {
                node.set_input(input);
            }
            if let Some(tag) = tag {
                node.set_tag(tag);
            }
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
            parser.namespace(format!("{}:{}", parser.nodes.len(), extension)),
        ) {
            attribute_type.parse(&mut parser, input.unwrap_or_default());

            // Drain any dispatches before trying to load the rest of the resources
            if let Some(mut storage) = parser.storage_mut() {
                storage.drain_dispatch_queues();
            }

            let rk = parser.parsed_node.last().copied();
            // Extension has been loaded to a namespace
            parser = handler.on_load(parser, namespace, rk).await;

            parser.handlers.push(handler);

            // Drain any dispatches before trying to load the rest of the resources
            if let Some(mut storage) = parser.storage_mut() {
                storage.drain_dispatch_queues();
            }
        }

        Some(Box::pin(parser))
    }

    async fn unload(&mut self) {
        // Drain any dispatches before trying to load the rest of the resources
        if let Some(mut storage) = self.storage_mut() {
            storage.drain_dispatch_queues();
        }

        let handler = self.handlers.last().cloned();
        if handler.is_some() {
            let parser = self.clone();
            *self = handler.unwrap().on_unload(parser).await;
        } else {
            if let Some(link_recv) = self.link_recv.pop() {
                let fields = self.fields.len();

                if let Some(last) = self.nodes.iter().rev().skip(fields).next() {
                    if let Ok(recv) = link_recv(last.clone(), self.fields.clone()).await {
                        trace!("Unloading node, setting recv from last link_recv");
                        self.parsed_node.node.set_repr(recv);
                        self.fields.clear();
                        self.link_recv.clear();
                    }
                }
            }
        }
    }
}

#[allow(unused)]
mod test {
    use reality_derive::Reality;

    use super::*;
    use crate::prelude::*;
    use crate::Project;
    use crate::Shared;
    use crate::Workspace;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_parser() {
        // Define a test resource
        #[derive(Reality, Clone, Default, Debug)]
        #[reality(call = noop, plugin)]
        struct Test;

        async fn noop(_: &mut ThunkContext) -> anyhow::Result<()> {
            Ok(())
        }

        impl runir::prelude::Field<0> for Test {
            type ParseType = String;
            type ProjectedType = String;
            fn field_name() -> &'static str {
                "test"
            }
        }

        struct Example;

        impl runir::prelude::Recv for Example {
            fn symbol() -> &'static str {
                "example"
            }
        }

        let mut project = Project::new(Shared::default());

        project.add_node_plugin("example", |input, tag, parser| {
            parser.add_type_with(
                "test",
                |parser, input| {
                    assert_eq!(2, parser.nodes.len(), "should be 2 nodes");

                    if let Some(node) = parser.nodes.last() {
                        let (symbol, input, tag, path, doc_headers, annotations) = node.mount();
                        assert_eq!(symbol.unwrap().as_str(), "test");
                        assert!(input.is_some());
                        assert_eq!(tag.unwrap().as_str(), "world");
                        assert_eq!(
                            annotations.unwrap().get("description").unwrap().as_str(),
                            "A really cool description"
                        );
                        eprintln!("{:?}", path);
                    }

                    if let Some(node) = parser.nodes.first() {
                        eprintln!("-{:?}", node.mount());
                    }
                },
                |n, f| {
                    Box::pin(async move {
                        let mut repr = Linker::new::<()>();
                        repr.push_level(RecvLevel::new::<Example>(f))?;
                        repr.push_level(n)?;

                        repr.link().await
                    })
                },
                |r, f, n| {
                    Box::pin(async move {
                        eprintln!("---{:?}", r.mount());
                        eprintln!("---{:?}", f.mount());
                        eprintln!("---{:?}", n.mount());
                        let mut factory = Linker::<CrcInterner>::default();
                        factory.push_level(r)?;
                        factory.push_level(f)?;
                        factory.push_level(n)?;
                        factory.link().await
                    })
                },
                ResourceLevel::new::<String>(),
                FieldLevel::new::<0, Test>(),
            );

            parser.with_object_type::<Test>();
        });

        let mut workspace = Workspace::new();

        workspace.add_buffer(
            "test.md",
            r#"
    ```runmd
    # -- Example of adding a node
    + .example hello-world
    |# description = A really cool description about an example

    # -- Example of defining a property called `test`
    : world .test hello
    |# name = test
    |# description = A really cool description

    # -- Example of adding an extension to the current node
    <reality.test>  hello-ext
    |# description = A really cool description of an extension
    ```
    "#,
        );

        let workspace = workspace.compile(project).await.unwrap();
        let project = workspace.project.unwrap();

        let nodes = project.nodes.read().await;

        for (node, store) in nodes.clone().iter() {
            let mut store = store.read().await;

            let attributes = store.resource::<ParsedNode>(ResourceKey::root()).unwrap();

            if let Some(node) = attributes.node.repr() {
                eprintln!("{:#}", node);
            }

            eprintln!("{:#?}", attributes);
        }

        ()
    }
}
