use std::marker::PhantomData;

use once_cell::sync::OnceCell;

use crate::AsyncStorageTarget;
use crate::Attribute;
use crate::AttributeParser;
use crate::AttributeType;
use crate::BlockObject;
use crate::Extension;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;

/// Trait for adding a hook to insert an extension for a plugin extension,
/// 
pub trait ExtensionController<Bob: BlockObject<Shared> + Send + Sync + 'static>: Sized + Send + Sync + 'static {
    /// Returns the identifier of this controller
    /// 
    fn ident() -> &'static str;

    /// Setup a new extension,
    /// 
    /// **Note**: If `Self: Default`, `Self::default_setup(..)` can be used to initialize a new extension,
    /// 
    fn setup(resource_key: Option<&ResourceKey<Attribute>>) -> Extension<Self, Bob>;

    /// Default extension setup,
    /// 
    fn default_setup(resource_key: Option<&ResourceKey<Attribute>>)  -> Extension<Self, Bob>
    where
        Self: Default {
            Extension::new(Self::default(), resource_key.map(|r| r.transmute()))
    }
}

/// Wrapper struct for injecting a plugin extension,
///
pub struct ExtensionPlugin<C, Bob>
where
    C: ExtensionController<Bob> + Send + Sync + 'static,
    Bob: BlockObject<Shared> + Send + Sync + 'static,
{
    _c: PhantomData<C>,
    _p: PhantomData<Bob>,
}

impl<C, Bob> std::str::FromStr for ExtensionPlugin<C, Bob>
where
    C: ExtensionController<Bob> + Send + Sync + 'static,
    Bob: BlockObject<Shared> + Send + Sync + 'static,
{
    type Err = anyhow::Error;

    fn from_str(_: &str) -> Result<Self, Self::Err> {
        Ok(ExtensionPlugin {
            _p: PhantomData,
            _c: PhantomData,
        })
    }
}

/// Attribute-type for the extension plugin,
///
impl<C, Bob> AttributeType<Shared> for ExtensionPlugin<C, Bob>
where
    C: ExtensionController<Bob> + Send + Sync + 'static,
    Bob: BlockObject<Shared> + Send + Sync + 'static,
{
    fn ident() -> &'static str {
        static VALUE: OnceCell<String> = once_cell::sync::OnceCell::new();

        VALUE.get_or_init(|| format!("{}({})", C::ident(), Bob::ident()))
    }

    fn parse(parser: &mut AttributeParser<Shared>, content: impl AsRef<str>) {
        Bob::parse(parser, content);

        let key = parser.attributes.last().clone();
        if let Some(storage) = parser.storage() {
            storage.lazy_put_resource::<Extension<C, Bob>>(
                C::setup(key),
                key.map(|k| k.transmute()),
            );
        }
    }
}

#[runmd::prelude::async_trait]
impl<C, Bob> BlockObject<Shared> for ExtensionPlugin<C, Bob>
where
    C: ExtensionController<Bob> + Send + Sync + 'static,
    Bob: BlockObject<Shared> + Send + Sync + 'static,
{
    /// Called when the block object is being loaded into it's namespace,
    ///
    async fn on_load(storage: AsyncStorageTarget<Shared>) {
        <Bob as BlockObject<Shared>>::on_load(storage).await;
    }

    /// Called when the block object is being unloaded from it's namespace,
    ///
    async fn on_unload(storage: AsyncStorageTarget<Shared>) {
        <Bob as BlockObject<Shared>>::on_unload(storage).await;
    }

    /// Called when the block object's parent attribute has completed processing,
    ///
    fn on_completed(storage: AsyncStorageTarget<Shared>) -> Option<AsyncStorageTarget<Shared>> {
        <Bob as BlockObject<Shared>>::on_completed(storage)
    }
}
