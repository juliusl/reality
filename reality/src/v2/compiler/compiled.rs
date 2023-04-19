use super::BuildLog;
use crate::state::Load;
use crate::state::Provider;
use crate::state::iter_state;
use crate::v2::Listen;
use crate::v2::ThunkCompile;
use crate::v2::ThunkListen;
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

/// Compiled data,
///
#[derive(SystemData)]
pub struct Compiled<'a> {
    /// Entity storage,
    ///
    entities: Entities<'a>,
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
    /// Thunk listen storage,
    /// 
    thunk_listens: ReadStorage<'a, ThunkListen>,
    /// Thunk listen storage,
    /// 
    thunk_compiles: ReadStorage<'a, ThunkCompile>,
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

    /// Visits objects in each build,
    /// 
    pub fn visit_builds(&self, visitor: &mut impl Visitor) {
        for (_, build) in iter_state::<Build, _>(self) {
            let build_log = build.build_log;
            for (_, entity) in build_log.index().iter() {
                self.visit_object(*entity, visitor);
            }
        }
    }

    /// Visits objects for a specific build,
    /// 
    pub fn visit_build(&self, build: Entity, visitor: &mut impl Visitor) {
        if let Some(build) = self.state::<Build>(build) {
            let build_log = build.build_log;
            for (_, entity) in build_log.index().iter() {
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
    pub fn update<T>(&self, updating: Entity, update: &impl Update<T>) -> Result<(), Error> {
        update.update(updating, self.lazy_update())
    }

    /// Returns a reference to lazy update resource,
    ///
    pub fn lazy_update(&self) -> &LazyUpdate {
        &self.lazy_updates
    }
}

/// Compiled object struct,
///
pub struct ObjectData<'a> {
    /// Entity,
    /// 
    pub entity: Entity,
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
    /// Thunk'ed listen fn,
    /// 
    listen: Option<&'a ThunkListen>,
    /// Thunk'ed compile fn,
    /// 
    compile: Option<&'a ThunkCompile>,
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
    /// Returns the current entity for this object,
    /// 
    pub fn entity(&self) -> Entity {
        match self {
            Object::Block(d) | Object::Root(d) | Object::Extension(d) => d.entity,
        }
    }

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

    /// Returns the thunk listen if one exists,
    /// 
    pub fn as_listen(&self) -> Option<&ThunkListen> {
        match self {
            Object::Block(d) | Object::Root(d) | Object::Extension(d) => d.listen,
        }
    }

    /// Returns the thunk compile if one exists,
    /// 
    pub fn as_compile(&self) -> Option<&ThunkCompile> {
        match self {
            Object::Block(d) | Object::Root(d) | Object::Extension(d) => d.compile,
        }
    }

    /// If object has a Thunk call and listen component, will execute the call thunk and pass the result to the
    /// listen thunk
    /// 
    pub async fn call_listen(&self, lazy_update: &LazyUpdate) -> Result<(), Error> {
        match (self.as_call(), self.as_listen()) {
            (Some(call), Some(listen)) => {
                let properties = call.call().await?;

                listen.listen(properties, lazy_update).await
            },
            _ => {
                Err("Object does not have a call and listen thunk".into())
            }
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
    MaybeJoin<&'a ReadStorage<'a, ThunkListen>>,
    MaybeJoin<&'a ReadStorage<'a, ThunkCompile>>,
);

impl<'a> Load for Object<'a> {
    type Layout = ObjectFormat<'a>;

    fn load(
        entity: Entity,
        (identifier, properties, block, root, call, build, update, listen, compile): <Self::Layout as Join>::Type,
    ) -> Self {
        let object_data = ObjectData {
            entity,
            identifier,
            properties,
            block,
            root,
            call,
            build,
            update,
            listen,
            compile,
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

/// Struct to load a view of a build,
/// 
#[derive(Clone)]
pub struct Build<'a> {
    /// Log of entities created by this build,
    /// 
    pub build_log: &'a BuildLog,
    /// Optional, identifier naming this build,
    /// 
    pub identifier: Option<&'a Identifier>,
    /// Optional, readonly, properties which contain properties for each entity created by this build,
    /// 
    pub properties: Option<&'a Properties>,
}

/// Type-alias for storage format of a single build,
/// 
pub type BuildFormat<'a> = (
    &'a ReadStorage<'a, BuildLog>,
    MaybeJoin<&'a ReadStorage<'a, Identifier>>,
    MaybeJoin<&'a ReadStorage<'a, Properties>>,
);

impl<'a> Load for Build<'a> {
    type Layout = BuildFormat<'a>;

    fn load(_: Entity, (build_log, identifier, properties): <Self::Layout as Join>::Type) -> Self {
        Build { build_log, identifier, properties }
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
            self.thunk_listens.maybe(),
            self.thunk_compiles.maybe(),
        )
    }
}

impl<'a> Provider<'a, BuildFormat<'a>> for Compiled<'a> {
    fn provide(&'a self) -> BuildFormat<'a> {
        (
            &self.build_logs,
            self.identifier.maybe(),
            self.properties.maybe(),
        )
    }
}

impl<'a> AsRef<Entities<'a>> for Compiled<'a> {
    fn as_ref(&self) -> &Entities<'a> {
        &self.entities
    }
}
