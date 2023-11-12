use std::collections::BTreeMap;
use std::collections::HashSet;
use std::marker::PhantomData;
use once_cell::sync::OnceCell;

use crate::AsyncStorageTarget;
use crate::Attribute;
use crate::AttributeParser;
use crate::AttributeType;
use crate::BlockObject;
use crate::CallAsync;
use crate::Transform;
use crate::Plugin;
use crate::ResourceKey;
use crate::Shared;
use crate::StorageTarget;
use crate::ThunkContext;

/// Trait for applying transforms to a plugin,
/// 
pub trait SetupTransform<Bob: BlockObject<Shared> + Send + Sync + 'static>:
    Sized + Send + Sync + 'static
{
    /// Returns the identifier of this controller
    ///
    fn ident() -> &'static str;

    /// Setup a new transform,
    ///
    /// **Note**: If `Self: Default`, `Self::default_setup(..)` can be used to initialize a new extension,
    ///
    fn setup_transform(resource_key: Option<&ResourceKey<Attribute>>) -> Transform<Self, Bob>;

    /// Default transform setup,
    ///
    fn default_setup(resource_key: Option<&ResourceKey<Attribute>>) -> Transform<Self, Bob>
    where
        Self: Default,
    {
        Transform::new(Self::default(), resource_key.map(|r| r.transmute()))
    }
}

/// Wrapper struct containing settings for applying a Transform,
///
pub struct TransformPlugin<C, Bob>
where
    C: SetupTransform<Bob> + Send + Sync + 'static,
    Bob: BlockObject<Shared> + Send + Sync + 'static,
{
    _c: PhantomData<C>,
    _p: PhantomData<Bob>,
    _s: OnceCell<String>,
}

impl<C, Bob> std::str::FromStr for TransformPlugin<C, Bob>
where
    C: SetupTransform<Bob> + Send + Sync + 'static,
    Bob: BlockObject<Shared> + Send + Sync + 'static,
{
    type Err = anyhow::Error;

    fn from_str(_: &str) -> Result<Self, Self::Err> {
        Ok(TransformPlugin {
            _p: PhantomData,
            _c: PhantomData,
            _s: OnceCell::new(),
        })
    }
}

/// Attribute-type for the transform plugin,
///
impl<C, Bob> AttributeType<Shared> for TransformPlugin<C, Bob>
where
    C: SetupTransform<Bob> + Send + Sync + 'static,
    Bob: BlockObject<Shared> + Send + Sync + 'static,
{
    fn symbol() -> &'static str {
        static mut SYMBOL_TABLE: BTreeMap<String, OnceCell<String>> = BTreeMap::new();
        let key = std::any::type_name::<Self>();

        unsafe {
            if let Some(c) = SYMBOL_TABLE.get(key) {
                return c.get_or_init(|| format!("({} {})", C::ident(), Bob::symbol()));
            } else {
                SYMBOL_TABLE.insert(key.to_string(), OnceCell::new());
                Self::symbol()
            }
        }
    }

    fn parse(parser: &mut AttributeParser<Shared>, content: impl AsRef<str>) {
        Bob::parse(parser, content);

        let key = parser.attributes.last();
        if let Some(storage) = parser.storage() {
            storage
                .lazy_put_resource::<Transform<C, Bob>>(C::setup_transform(key), key.map(|k| k.transmute()));
        }
    }
}

#[runmd::prelude::async_trait]
impl<C, Bob> BlockObject<Shared> for TransformPlugin<C, Bob>
where
    C: SetupTransform<Bob> + Send + Sync + 'static,
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

#[async_trait::async_trait]
impl<C, P> CallAsync for TransformPlugin<C, P>
where
    C: SetupTransform<P> + Send + Sync + 'static,
    P: Plugin + Clone + Default + Send + Sync + 'static,
{
    /// Executed by `ThunkContext::spawn`,
    ///
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        use std::ops::DerefMut;
        let mut ext = C::setup_transform(context.attribute.as_ref());
        let initialized = context.initialized::<P>().await;
        let tag = context.tag().await;

        // Branch context to avoid mutating the original directly
        let (variant_id, mut context) = context.branch();

        let initialized = { ext.run(&mut context, initialized).await? };

        // Insert the modified value as the initialized state before calling the next plugin
        unsafe {
            let mut source = context.node_mut().await;
            source.put_resource(initialized, context.attribute.map(|a| a.transmute()));

            if let Some(tag) = tag {
                source.put_resource(tag, context.attribute.map(|a| a.transmute()));
            }

            // Track variants that branched from this point
            let controller = Some(ResourceKey::with_hash(C::ident()));
            if !borrow_mut!(source, HashSet<uuid::Uuid>, controller, |list| => {
                list.insert(variant_id);
            }) {
                let mut set = HashSet::new();
                set.insert(variant_id);
                source.put_resource(set, controller);
            }
        }

        let result = <P as CallAsync>::call(&mut context).await;
        context.garbage_collect();

        result
    }
}

impl<C, P> Plugin for TransformPlugin<C, P>
where
    C: SetupTransform<P> + Send + Sync + 'static,
    P: Plugin + Clone + Default + Send + Sync + 'static,
{
}
