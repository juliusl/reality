use crate::AsyncStorageTarget;
use crate::Attribute;
use crate::AttributeType;
use crate::AttributeTypeParser;
use crate::FieldPacket;
use crate::ResourceKey;
use crate::SetField;
use crate::StorageTarget;
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

    /// Returns an empty handler for this block object,
    ///
    fn handler() -> BlockObjectHandler<Storage::Namespace> {
        BlockObjectHandler::<Storage::Namespace>::new::<Storage, Self>()
    }

    /// Called when the block object is being loaded into it's namespace,
    ///
    async fn on_load(
        storage: AsyncStorageTarget<Storage::Namespace>,
        rk: Option<ResourceKey<Attribute>>,
    );

    /// Called when the block object is being unloaded from it's namespace,
    ///
    async fn on_unload(
        storage: AsyncStorageTarget<Storage::Namespace>,
        rk: Option<ResourceKey<Attribute>>,
    );

    /// Called when the block object's parent attribute has completed processing,
    ///
    fn on_completed(
        storage: AsyncStorageTarget<Storage::Namespace>,
    ) -> Option<AsyncStorageTarget<Storage::Namespace>>;
}

/// Type-alias for a block object event fn,
///
type BlockObjectFn<Storage> =
    fn(
        AsyncStorageTarget<Storage>,
        Option<ResourceKey<Attribute>>,
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
    resource_key: Option<ResourceKey<Attribute>>,
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
            resource_key: self.resource_key.clone(),
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
            resource_key: None,
        }
    }

    /// Calls the on_load handler,
    ///
    pub async fn on_load(
        &mut self,
        namespace: AsyncStorageTarget<Storage>,
        key: Option<ResourceKey<Attribute>>,
    ) {
        (self.on_load)(namespace.clone(), key.clone()).await;
        self.namespace = Some(namespace);
        self.resource_key = key;
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
            (self.on_unload)(namespace, self.resource_key).await
        }
    }
}
