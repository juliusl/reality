use std::collections::BTreeMap;
use super::BuildLog;
use crate::state::Load;
use crate::state::Provider;
use crate::v2::thunk::ThunkUpdate;
use crate::v2::thunk::Update;
use crate::v2::Block;
use crate::v2::Call;
use crate::v2::Properties;
use crate::v2::Root;
use crate::v2::ThunkBuild;
use crate::v2::ThunkCall;
use crate::v2::Visitor;
use crate::Error;
use crate::Identifier;
use specs::join::MaybeJoin;
use specs::prelude::*;
use specs::ReadStorage;
use specs::SystemData;
use tracing::trace;

/// Compiled runmd data,
///
#[derive(SystemData)]
pub struct Compiled<'a> {
    /// Entity storage,
    ///
    entities: Entities<'a>,
    /// Tokio runtime handle,
    ///
    tokio_handle: Read<'a, Option<tokio::runtime::Handle>>,
    /// Lazy update resource,
    ///
    lazy_updates: Read<'a, LazyUpdate>,
    /// Identifier storage,
    ///
    identifier: ReadStorage<'a, Identifier>,
    /// Properties storage,
    ///
    properties: ReadStorage<'a, Properties>,
    /// Block storage,
    ///
    blocks: ReadStorage<'a, Block>,
    /// Root storage,
    ///
    roots: ReadStorage<'a, Root>,
    /// Thunk call storage,
    ///
    thunk_calls: ReadStorage<'a, ThunkCall>,
    /// Thunk build storage,
    ///
    thunk_builds: ReadStorage<'a, ThunkBuild>,
    /// Thunk update storage,
    ///
    thunk_updates: ReadStorage<'a, ThunkUpdate>,
    /// Build log storage,
    ///
    build_logs: ReadStorage<'a, BuildLog>,
}

impl<'a> Compiled<'a> {
    /// Finds a build log,
    ///
    pub fn find_build(&self, build: Entity) -> Option<&BuildLog> {
        self.build_logs.get(build)
    }

    /// Visits the objects created by a build,
    ///
    pub fn visit_build(&self, build: Entity, visitor: &mut impl Visitor) {
        if let Some(build_log) = self.find_build(build) {
            for (ident, entity) in build_log.index().iter() {
                trace!("Visiting {:#}", ident);
                self.visit_object(*entity, visitor);
            }
        }
    }

    /// Visits an object and returns the object visited if successful,
    ///
    pub fn visit_object(&self, object: Entity, visitor: &mut impl Visitor) -> Option<Object> {
        if let Some(obj) = self.state::<Object>(object) {
            visitor.visit_object(&obj);
            self.state::<Object>(object)
        } else {
            None
        }
    }

    /// Updates an entity w/ a type that implements the Update trait,
    ///
    pub fn update(&self, updating: Entity, update: &impl Update) -> Result<(), Error> {
        update.update(updating, self.lazy_update())
    }

    /// Looks for an indexed object compiled from a build by identifier.
    /// If found and a ThunkCall component exists, the component will be cloned and invoked, returning the result.
    ///
    /// Otherwise, if the object/build/thunk component do not exist, an error w/ message is returned.
    ///
    /// # Notes on identifier
    /// An object identifier is committed before being inserted into a build log index. Which means the full identifier
    /// must be specified in order for it to be found in the build log.
    ///
    /// One tiny detail is that the identifier component stored w/ the object maintains the hierarchy,
    /// which means identifiers gathered w/ a visitor have not yet been committed. Using this method address that issue, since the
    /// search_index fn will call commit before trying to get the entity from the index.
    ///
    pub async fn call_from(&self, build: Entity, ident: &Identifier) -> Result<Properties, Error> {
        if let Some(thunk) = self.find_build(build).map(|log| log.get(ident)) {
            let thunk = thunk?;
            self.call(thunk).await.map_err(|_| {
                format!(
                    "Either the object, or thunk do not exist w/ identifier {:?}",
                    ident
                )
                .into()
            })
        } else {
            Err(format!("Build {:?} no longer exists", build).into())
        }
    }

    /// Searches for call thunks from a build log w/ string interpolation pattern and prepares a batch call joinset,
    ///
    /// The join set will return results in the order tasks complete,
    ///
    pub fn batch_call_matches(
        &self,
        build: Entity,
        pat: impl Into<String>,
    ) -> Result<BatchCallJoinSet, Error> {
        if let Some((build_log, handle)) = self.find_build(build).zip(self.tokio_handle.as_ref()) {
            let mut joinset = tokio::task::JoinSet::<
                Result<(Identifier, BTreeMap<String, String>, Properties), Error>,
            >::new();

            for (ident, map, e) in build_log.search(pat) {
                let thunk = self.state::<Object>(*e).and_then(|o| o.as_call().cloned());
                if let Some(thunk) = thunk {
                    let ident = ident.clone();
                    joinset.spawn_on(
                        async move {
                            let result = thunk.call().await?;

                            Ok((ident, map, result))
                        },
                        handle,
                    );
                }
            }

            Ok(joinset)
        } else {
            Err("Could not find existing build or tokio runtime handle".into())
        }
    }

    /// Returns the result of a thunk call component stored on thunk_entity,
    ///
    pub async fn call(&self, thunk_entity: Entity) -> Result<Properties, Error> {
        let thunk = self
            .state::<Object>(thunk_entity)
            .and_then(|o| o.as_call().cloned());

        if let Some(thunk) = thunk {
            thunk.call().await
        } else {
            Err(format!(
                "Either the object, or thunk do not exist on entity {:?}",
                thunk_entity
            )
            .into())
        }
    }

    /// Returns a reference to lazy update resource,
    ///
    pub fn lazy_update(&self) -> &LazyUpdate {
        &self.lazy_updates
    }
}

/// Type-alias for a batch call join-set,
///
pub type BatchCallJoinSet =
    tokio::task::JoinSet<Result<(Identifier, BTreeMap<String, String>, Properties), Error>>;

/// Compiled object struct,
///
pub struct ObjectData<'a> {
    /// Object's identifier,
    ///
    identifier: &'a Identifier,
    /// Block properties,
    ///
    properties: &'a Properties,
    /// Compiled source block,
    ///
    block: Option<&'a Block>,
    /// Compiled root,
    ///
    root: Option<&'a Root>,
    /// Thunk'ed call fn,
    ///
    call: Option<&'a ThunkCall>,
    /// Thunk'ed build fn,
    ///
    build: Option<&'a ThunkBuild>,
    /// Thunk'ed update fn,
    ///
    update: Option<&'a ThunkUpdate>,
}

/// Enumeration of compiled object hierarchy,
///
/// Hierarchy:
///
/// Block
///      |
///      ---> Root
///              |
///              ----> Extensions
///
/// All objects have at least an identifier and a properties map. In addition any of the thunk types, can be installed
/// w/ any object variant, and will be returned w/ the object when loaded.
///
pub enum Object<'a> {
    /// A block variant means that the object will have a block component,
    ///
    /// A block component will contain a vector of roots,
    ///
    Block(ObjectData<'a>),
    /// A root variant means that the object will have a root component,
    ///
    /// A root component will contain a vector of extension identifiers,
    ///
    Root(ObjectData<'a>),
    /// If the object is neither a block or root, it is considered an extension variant,
    ///
    /// An extension variant will only have an identifier, whose ancestors will point to the root and block origins,
    /// and properties that were parsed on compilation.
    ///
    /// The actual use of an extension is to be defined by consumers of the library.
    ///
    Extension(ObjectData<'a>),
}

impl<'a> Object<'a> {
    /// Returns the object identifier,
    ///
    pub fn ident(&self) -> &Identifier {
        match self {
            Object::Block(d) | Object::Root(d) | Object::Extension(d) => d.identifier,
        }
    }

    /// Returns the properties associated w/ this object,
    ///
    pub fn properties(&self) -> &Properties {
        match self {
            Object::Block(d) | Object::Root(d) | Object::Extension(d) => d.properties,
        }
    }

    /// Returns true if this object is a root,
    ///
    pub fn is_root(&self) -> bool {
        match self {
            Object::Root(_) => true,
            _ => false,
        }
    }

    /// Returns true if this object is a block,
    ///
    pub fn is_block(&self) -> bool {
        match self {
            Object::Block(_) => true,
            _ => false,
        }
    }

    /// Returns true if this object is a root extension,
    ///
    pub fn is_extension(&self) -> bool {
        match self {
            Object::Extension(_) => true,
            _ => false,
        }
    }

    /// Returns this object as a root attribute,
    ///
    pub fn as_root(&self) -> Option<&Root> {
        match self {
            Object::Root(d) => d.root,
            _ => None,
        }
    }

    /// Returns this object as a block,
    ///
    pub fn as_block(&self) -> Option<&Block> {
        match self {
            Object::Block(d) => d.block,
            _ => None,
        }
    }

    /// Returns the thunk call if one exists,
    ///
    pub fn as_call(&self) -> Option<&ThunkCall> {
        match self {
            Object::Block(d) | Object::Root(d) | Object::Extension(d) => d.call,
        }
    }

    /// Returns the thunk build if one exists,
    ///
    pub fn as_build(&self) -> Option<&ThunkBuild> {
        match self {
            Object::Block(d) | Object::Root(d) | Object::Extension(d) => d.build,
        }
    }

    /// Returns the thunk update if one exists,
    ///
    pub fn as_update(&self) -> Option<&ThunkUpdate> {
        match self {
            Object::Block(d) | Object::Root(d) | Object::Extension(d) => d.update,
        }
    }
}

/// Object data format,
///
pub type ObjectFormat<'a> = (
    &'a ReadStorage<'a, Identifier>,
    &'a ReadStorage<'a, Properties>,
    MaybeJoin<&'a ReadStorage<'a, Block>>,
    MaybeJoin<&'a ReadStorage<'a, Root>>,
    MaybeJoin<&'a ReadStorage<'a, ThunkCall>>,
    MaybeJoin<&'a ReadStorage<'a, ThunkBuild>>,
    MaybeJoin<&'a ReadStorage<'a, ThunkUpdate>>,
);

impl<'a> Load for Object<'a> {
    type Layout = ObjectFormat<'a>;

    fn load(
        (identifier, properties, block, root, call, build, update): <Self::Layout as Join>::Type,
    ) -> Self {
        let object_data = ObjectData {
            identifier,
            properties,
            block,
            root,
            call,
            build,
            update,
        };

        if block.is_some() {
            Object::<'a>::Block(object_data)
        } else if root.is_some() {
            Object::<'a>::Root(object_data)
        } else {
            Object::<'a>::Extension(object_data)
        }
    }
}

impl<'a> Provider<'a, ObjectFormat<'a>> for Compiled<'a> {
    fn provide(&'a self) -> ObjectFormat<'a> {
        (
            &self.identifier,
            &self.properties,
            self.blocks.maybe(),
            self.roots.maybe(),
            self.thunk_calls.maybe(),
            self.thunk_builds.maybe(),
            self.thunk_updates.maybe(),
        )
    }
}

impl<'a> AsRef<Entities<'a>> for Compiled<'a> {
    fn as_ref(&self) -> &Entities<'a> {
        &self.entities
    }
}
