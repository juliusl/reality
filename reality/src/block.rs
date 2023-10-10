use std::str::FromStr;

use reality_derive::AttributeType;

use crate::AsyncStorageTarget;
use crate::AttributeParser;
use crate::AttributeType;
use crate::AttributeTypeParser;
use crate::ResourceStorageConfig;
use crate::StorageTarget;

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
            ident: <B as AttributeType<Storage>>::ident(),
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

/// Object type that lives inside of a runmd block,
/// 
#[crate::runmd::async_trait]
pub trait BlockObject<Storage>: AttributeType<Storage>
where
    Self: Sized + Send + Sync + 'static,
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
    fn on_completed(storage: AsyncStorageTarget<Storage::Namespace>) -> Option<AsyncStorageTarget<Storage::Namespace>>;

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
            on_load: self.on_load.clone(),
            on_unload: self.on_unload.clone(),
            on_completed: self.on_completed.clone(),
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
    pub fn new<BlockStorage: StorageTarget<Namespace = Storage> + 'static, B: BlockObject<BlockStorage>>() -> BlockObjectHandler<BlockStorage::Namespace> 
    where 
    {
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
        let dispatcher = storage.intialize_dispatcher::<()>(ResourceStorageConfig::new()).await;
        let mut storage = storage.storage.write().await;
        storage.put_resource(dispatcher, ResourceStorageConfig::new());
    }

    async fn on_unload(storage: AsyncStorageTarget<Storage::Namespace>) {
        let mut disp = storage.dispatcher::<u64>(ResourceStorageConfig::new()).await;
        disp.dispatch_all().await;
    }

    fn on_completed(storage: AsyncStorageTarget<Storage::Namespace>) -> Option<AsyncStorageTarget<Storage::Namespace>> {
        Some(storage)
    }
}
