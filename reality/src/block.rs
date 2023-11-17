use std::collections::BTreeMap;
use std::str::FromStr;

use anyhow::anyhow;
use async_trait::async_trait;
use reality_derive::AttributeType;
use serde::Deserialize;

use crate::AsyncStorageTarget;
use crate::AttributeParser;
use crate::AttributeType;
use crate::AttributeTypeParser;
use crate::FieldPacket;
use crate::Plugin;
use crate::SetField;
use crate::StorageTarget;
use crate::ThunkContext;
use crate::ToFrame;

/// Struct containing block object functions,
///
pub struct BlockObjectType<Storage>
where
    Storage: StorageTarget + 'static,
{
    /// Attribute type ident,
    ///
    pub ident: &'static str,
    /// Attribute type parser,
    ///
    pub attribute_type: AttributeTypeParser<Storage>,
    /// Object event handlers,
    ///
    pub handler: BlockObjectHandler<Storage::Namespace>,
}

impl<Storage> BlockObjectType<Storage>
where
    Storage: StorageTarget + 'static,
{
    /// Creates a new block object type,
    ///
    pub fn new<B: BlockObject<Storage>>() -> Self {
        Self {
            ident: <B as AttributeType<Storage>>::symbol(),
            attribute_type: B::attribute_type(),
            handler: B::handler(),
        }
    }
}

impl<Storage: StorageTarget + 'static> Clone for BlockObjectType<Storage> {
    fn clone(&self) -> Self {
        Self {
            ident: self.ident,
            attribute_type: self.attribute_type.clone(),
            handler: self.handler.clone(),
        }
    }
}

pub trait SetIdentifiers {
    fn set_identifiers(&mut self, name: &str, tag: Option<&String>);
}

/// Trait for adding utilities for reading property data from a plugin
/// definition
///
#[async_trait]
pub trait PropertySource<T>
where
    T: Plugin,
{
    /// Property reader,
    ///
    type Reader;

    /// Returns a handler to the reader type,
    ///
    async fn properties(tc: &mut ThunkContext) -> anyhow::Result<Self::Reader> {
        let init = tc.initialized::<T>().await;
        // 1. Convert to frame
        // 2. Get the ParsedAttributes
        // 3. Assign comment properties
        // 4. Cache?
        /*
            Reader can
            - iter properties?
            - find updated values?
        */
        Self::reader(init, tc).await
    }

    /// Constructs a new property reader,
    ///
    async fn reader(init: T, tc: &mut ThunkContext) -> anyhow::Result<Self::Reader>;
}

/// Plain wrapper over T,
///
pub struct Field<T>(T);

/// Generic wrapper over inner field type to make generating
/// a reader easier,
///
pub struct PropertyReader<T>
where
    for<'de> T: Deserialize<'de>,
{
    value: T,
    _doc_headers: Vec<String>,
    _comment_properties: BTreeMap<String, String>,
}

impl<T> From<PropertyReader<T>> for Field<T>
where
    for<'de> T: Deserialize<'de>,
{
    fn from(value: PropertyReader<T>) -> Self {
        Field(value.value)
    }
}

impl<T> From<T> for PropertyReader<T>
where
    for<'de> T: Deserialize<'de>,
{
    fn from(value: T) -> Self {
        Self {
            value,
            _doc_headers: Vec::default(),
            _comment_properties: BTreeMap::default(),
        }
    }
}

impl<T> TryFrom<&FieldPacket> for PropertyReader<T>
where
    for<'de> T: Deserialize<'de>,
{
    type Error = anyhow::Error;

    fn try_from(value: &FieldPacket) -> Result<Self, Self::Error> {
        Self::read_packet(value)
    }
}

impl<T> PropertyReader<T>
where
    for<'de> T: Deserialize<'de>,
{
    /// Read a field packet as some type T,
    ///
    fn read_packet(field: &FieldPacket) -> anyhow::Result<PropertyReader<T>> {
        if let Some(wire_data) = &field.wire_data {
            Ok(PropertyReader {
                value: bincode::deserialize::<T>(wire_data)?,
                _doc_headers: Vec::default(),
                _comment_properties: BTreeMap::default(),
            })
        } else {
            Err(anyhow!("Packet has no wire data"))
        }
    }

    pub fn with_comment_properties(mut self, properties: BTreeMap<String, String>) -> Self {
        self._comment_properties = properties;
        self
    }
}

/// Object type that lives inside of a runmd block,
///
#[crate::runmd::async_trait]
pub trait BlockObject<Storage>: AttributeType<Storage>
where
    Self: SetField<FieldPacket> + ToFrame + Sized + Send + Sync + 'static,
    Storage: StorageTarget + 'static,
{
    /// Returns the attribute-type parser for the block-object type,
    ///
    fn attribute_type() -> AttributeTypeParser<Storage> {
        AttributeTypeParser::new::<Self>()
    }

    /// Called when the block object is being loaded into it's namespace,
    ///
    async fn on_load(storage: AsyncStorageTarget<Storage::Namespace>);

    /// Called when the block object is being unloaded from it's namespace,
    ///
    async fn on_unload(storage: AsyncStorageTarget<Storage::Namespace>);

    /// Called when the block object's parent attribute has completed processing,
    ///
    fn on_completed(
        storage: AsyncStorageTarget<Storage::Namespace>,
    ) -> Option<AsyncStorageTarget<Storage::Namespace>>;

    /// Returns an empty handler for this block object,
    ///
    fn handler() -> BlockObjectHandler<Storage::Namespace> {
        BlockObjectHandler::<Storage::Namespace>::new::<Storage, Self>()
    }
}

/// Type-alias for a block object event fn,
///
type BlockObjectFn<Storage> =
    fn(
        AsyncStorageTarget<Storage>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>;

/// Type-alias for a block object event completion fn,
///
type BlockObjectCompletionFn<Storage> =
    fn(parser: AsyncStorageTarget<Storage>) -> Option<AsyncStorageTarget<Storage>>;

/// Concrete trait type for a type that implements BlockObject,
///
pub struct BlockObjectHandler<Storage>
where
    Storage: StorageTarget + 'static,
{
    on_load: BlockObjectFn<Storage>,
    on_unload: BlockObjectFn<Storage>,
    on_completed: BlockObjectCompletionFn<Storage>,
    namespace: Option<AsyncStorageTarget<Storage>>,
}

impl<Storage> Clone for BlockObjectHandler<Storage>
where
    Storage: StorageTarget + 'static,
{
    fn clone(&self) -> Self {
        Self {
            on_load: self.on_load,
            on_unload: self.on_unload,
            on_completed: self.on_completed,
            namespace: self.namespace.clone(),
        }
    }
}

impl<Storage> BlockObjectHandler<Storage>
where
    Storage: StorageTarget + 'static,
{
    /// Creates a new function resource from a block object,
    ///
    pub fn new<
        BlockStorage: StorageTarget<Namespace = Storage> + 'static,
        B: BlockObject<BlockStorage>,
    >() -> BlockObjectHandler<BlockStorage::Namespace>
where {
        Self {
            on_load: B::on_load,
            on_unload: B::on_unload,
            on_completed: B::on_completed,
            namespace: None,
        }
    }

    /// Calls the on_load handler,
    ///
    pub async fn on_load(&mut self, namespace: AsyncStorageTarget<Storage>) {
        (self.on_load)(namespace.clone()).await;
        self.namespace = Some(namespace);
    }

    /// Calls the on_completed handler,
    ///
    pub fn on_completed(&self) -> Option<AsyncStorageTarget<Storage>> {
        if let Some(namespace) = self.namespace.clone() {
            (self.on_completed)(namespace)
        } else {
            None
        }
    }

    /// Calls the on_unload handler,
    ///
    pub async fn on_unload(&self) {
        if let Some(namespace) = self.namespace.clone() {
            (self.on_unload)(namespace).await
        }
    }
}

#[derive(AttributeType)]
struct Test {}

impl FromStr for Test {
    type Err = ();

    fn from_str(_: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

#[runmd::prelude::async_trait]
impl<Storage: StorageTarget + Send + Sync + 'static> BlockObject<Storage> for Test {
    async fn on_load(storage: AsyncStorageTarget<Storage::Namespace>) {
        let dispatcher = storage.intialize_dispatcher::<()>(None).await;
        let mut storage = storage.storage.write().await;
        storage.put_resource(dispatcher, None);
    }

    async fn on_unload(storage: AsyncStorageTarget<Storage::Namespace>) {
        let mut disp = storage.dispatcher::<u64>(None).await;
        disp.dispatch_all().await;
    }

    fn on_completed(
        storage: AsyncStorageTarget<Storage::Namespace>,
    ) -> Option<AsyncStorageTarget<Storage::Namespace>> {
        Some(storage)
    }
}

impl ToFrame for Test {
    fn to_frame(&self, key: Option<crate::ResourceKey<crate::Attribute>>) -> crate::Frame {
        crate::Frame {
            fields: vec![],
            recv: self.receiver_packet(key),
        }
    }
}

impl SetField<FieldPacket> for Test {
    fn set_field(&mut self, _: crate::FieldOwned<FieldPacket>) -> bool {
        false
    }
}

#[test]
fn test_property_reader() {
    let test = Test {};
    if let Some(test) = test.to_frame(None).fields.first() {
        let _reader = PropertyReader::<usize>::read_packet(test).unwrap();
    }
}
