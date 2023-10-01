use std::str::FromStr;

use reality_derive::AttributeType;

use crate::AsyncStorageTarget;
use crate::AttributeParser;
use crate::AttributeType;
use crate::AttributeTypePackage;
use crate::AttributeTypeParser;
use crate::ResourceKey;
use crate::StorageTarget;

/// Struct containing all attributes,
///
pub struct Block {}

pub trait BlockPackage<S: StorageTarget + 'static> {
    /// Resource key for the block package,
    ///
    fn resource_key() -> ResourceKey<AttributeTypePackage<S>>;

    /// Initialized package,
    ///
    fn package() -> AttributeTypePackage<S>;
}

/// Object type that lives inside of a runmd block,
///
/// Initiated w/ the `+` keyword,
///
#[runmd::prelude::async_trait]
pub trait BlockObject<Storage>: AttributeType<Storage>
where
    Self: Sized + Send + Sync + 'static,
    Storage: StorageTarget + Send + Sync + 'static,
{
    /// Called when the block object is being loaded,
    ///
    async fn on_load(storage: AsyncStorageTarget<Storage>);

    /// Called when the block object is being unloaded,
    ///
    async fn on_unload(storage: AsyncStorageTarget<Storage>);

    /// Called when the block object's parent attribute has completed processing,
    ///
    fn on_completed(storage: AsyncStorageTarget<Storage>) -> Option<AsyncStorageTarget<Storage>>;

    /// Returns the attribute-type parser for the block-object type,
    ///
    fn attribute_type() -> AttributeTypeParser<Storage> {
        AttributeTypeParser::new::<Self>()
    }

    /// Returns an empty handler for this block object,
    ///
    fn handler() -> BlockObjectHandler<Storage> {
        BlockObjectHandler::new::<Self>()
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
    Storage: StorageTarget + Send + Sync + 'static,
{
    on_load: BlockObjectFn<Storage>,
    on_unload: BlockObjectFn<Storage>,
    on_completed: BlockObjectCompletionFn<Storage>,
    namespace: Option<AsyncStorageTarget<Storage>>,
}

impl<Storage> Clone for BlockObjectHandler<Storage>
where
    Storage: StorageTarget + Send + Sync + 'static,
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
    Storage: StorageTarget + Send + Sync + 'static,
{
    /// Creates a new function resource from a block object,
    ///
    pub fn new<B: BlockObject<Storage>>() -> Self {
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
    async fn on_load(storage: AsyncStorageTarget<Storage>) {
        let dispatcher = storage.intialize_dispatcher::<()>(None).await;
        let mut storage = storage.storage.write().await;
        storage.put_resource(dispatcher, None);
    }

    async fn on_unload(storage: AsyncStorageTarget<Storage>) {
        let mut disp = storage.dispatcher::<u64>(None).await;
        disp.dispatch_all().await;
    }

    fn on_completed(storage: AsyncStorageTarget<Storage>) -> Option<AsyncStorageTarget<Storage>> {
        Some(storage)
    }
}
