use runir::prelude::Level;
use runir::prelude::ResourceLevel;
use tracing::trace;

use crate::AsyncStorageTarget;
use crate::Attribute;
use crate::AttributeParser;
use crate::AttributeType;
use crate::AttributeTypeParser;
use crate::FieldPacket;
use crate::LinkRecvFn;
use crate::ResourceKey;
use crate::SetField;
use crate::Shared;
use crate::ToFrame;

/// Struct containing block object functions,
///
pub struct BlockObjectType {
    /// Attribute type ident,
    ///
    pub ident: &'static str,
    /// Attribute type parser,
    ///
    pub attribute_type: AttributeTypeParser<Shared>,
    /// Object event handlers,
    ///
    pub handler: BlockObjectHandler,
}

impl BlockObjectType {
    /// Creates a new block object type,
    ///
    pub fn new<B: BlockObject>() -> Self {
        Self {
            ident: B::symbol(),
            attribute_type: B::attribute_type(),
            handler: B::handler(),
        }
    }

    pub fn new_as<B: BlockObject, As: BlockObject>() -> Self {
        Self {
            ident: B::symbol(),
            attribute_type: B::attribute_type_as::<As>(),
            handler: B::handler(),
        }
    }
}

impl Clone for BlockObjectType {
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
#[crate::runmd::async_trait(?Send)]
pub trait BlockObject
where
    Self: AttributeType<Shared> + SetField<FieldPacket> + ToFrame + Sized + Send + Sync + 'static,
{
    /// Returns the attribute-type parser for the block-object type,
    ///
    fn attribute_type() -> AttributeTypeParser<Shared> {
        AttributeTypeParser::new::<Self>(ResourceLevel::new::<Self>())
    }

    /// Returns the attribute-type parser for the block-object type replacing the inner type,
    ///
    fn attribute_type_as<Inner>() -> AttributeTypeParser<Shared>
    where
        Inner:
            AttributeType<Shared> + SetField<FieldPacket> + ToFrame + Sized + Send + Sync + 'static,
    {
        AttributeTypeParser::new::<Self>(ResourceLevel::new::<Inner>())
    }

    /// Returns an empty handler for this block object,
    ///
    fn handler() -> BlockObjectHandler {
        BlockObjectHandler::new::<Self>()
    }

    /// Called when the block object is being loaded into it's namespace,
    ///
    async fn on_load(
        parser: AttributeParser<Shared>,
        storage: AsyncStorageTarget<Shared>,
        rk: Option<ResourceKey<Attribute>>,
    ) -> AttributeParser<Shared>;

    /// Called when the block object is being unloaded from it's namespace,
    ///
    async fn on_unload(
        parser: AttributeParser<Shared>,
        storage: AsyncStorageTarget<Shared>,
        rk: Option<ResourceKey<Attribute>>,
    ) -> AttributeParser<Shared>;

    /// Called when the block object's parent attribute has completed processing,
    ///
    fn on_completed(storage: AsyncStorageTarget<Shared>) -> Option<AsyncStorageTarget<Shared>>;
}

/// Type-alias for a block object event fn,
///
type BlockObjectFn =
    fn(
        AttributeParser<Shared>,
        AsyncStorageTarget<Shared>,
        Option<ResourceKey<Attribute>>,
    ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = AttributeParser<Shared>>>>;

/// Type-alias for a block object event completion fn,
///
type BlockObjectCompletionFn =
    fn(parser: AsyncStorageTarget<Shared>) -> Option<AsyncStorageTarget<Shared>>;

/// Concrete trait type for a type that implements BlockObject,
///
pub struct BlockObjectHandler {
    on_load: BlockObjectFn,
    on_unload: BlockObjectFn,
    on_completed: BlockObjectCompletionFn,
    link_recv: LinkRecvFn,
    namespace: Option<AsyncStorageTarget<Shared>>,
    resource_key: Option<ResourceKey<Attribute>>,
}

impl Clone for BlockObjectHandler {
    fn clone(&self) -> Self {
        Self {
            on_load: self.on_load,
            on_unload: self.on_unload,
            on_completed: self.on_completed,
            link_recv: self.link_recv,
            namespace: self.namespace.clone(),
            resource_key: self.resource_key,
        }
    }
}

impl BlockObjectHandler {
    /// Creates a new function resource from a block object,
    ///
    pub fn new<B>() -> BlockObjectHandler
    where
        B: BlockObject,
    {
        Self {
            on_load: B::on_load,
            on_unload: B::on_unload,
            on_completed: B::on_completed,
            link_recv: B::link_recv,
            namespace: None,
            resource_key: None,
        }
    }

    /// Calls the on_load handler,
    ///
    pub async fn on_load(
        &mut self,
        parser: AttributeParser<Shared>,
        namespace: AsyncStorageTarget<Shared>,
        key: Option<ResourceKey<Attribute>>,
    ) -> AttributeParser<Shared> {
        let parser = (self.on_load)(parser, namespace.clone(), key).await;
        self.namespace = Some(namespace);
        self.resource_key = key;
        parser
    }

    /// Calls the on_completed handler,
    ///
    pub fn on_completed(&self) -> Option<AsyncStorageTarget<Shared>> {
        if let Some(namespace) = self.namespace.clone() {
            (self.on_completed)(namespace)
        } else {
            None
        }
    }

    /// Calls the on_unload handler,
    ///
    pub async fn on_unload(&self, parser: AttributeParser<Shared>) -> AttributeParser<Shared> {
        if let Some(namespace) = self.namespace.clone() {
            let mut parser = (self.on_unload)(parser, namespace, self.resource_key).await;

            for n in parser.nodes.iter() {
                trace!("* {:?}", n.mount());
            }

            let f = parser.fields.len();
            trace!("fields -- {f}");
            if let Some(recv) = parser.nodes.iter().rev().nth(f) {
                trace!("trying unloading -- {:?}", recv.mount());
                match (self.link_recv)(recv.clone(), parser.fields.clone()).await {
                    Ok(recv) => {
                        if let Some(rk) = parser.parsed_node.attributes.last_mut() {
                            rk.set_repr(recv);
                        }
                    }
                    Err(err) => {
                        eprintln!("{err}");
                    }
                }
            }
            parser.fields.clear();
            parser
        } else {
            panic!("could not unload parser")
        }
    }
}
