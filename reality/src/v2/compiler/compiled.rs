use super::BuildLog;
use crate::state::Load;
use crate::state::Provider;
use crate::v2::Properties;
use crate::v2::Root;
use crate::v2::Block;
use crate::Identifier;
use specs::join::MaybeJoin;
use specs::prelude::*;
use specs::ReadStorage;
use specs::SystemData;

/// Compiled runmd data,
///
#[derive(SystemData)]
pub struct Compiled<'a> {
    /// Entity storage,
    /// 
    entities: Entities<'a>,
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
}

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
pub enum Object<'a> {
    Block(ObjectData<'a>),
    Root(ObjectData<'a>),
    Extension(ObjectData<'a>),
}

impl<'a> Object<'a> {
    /// Returns the object identifier,
    /// 
    pub fn ident(&self) -> &Identifier {
        match self {
            Object::Block(d) |
            Object::Root(d) |
            Object::Extension(d) => d.identifier,
        }
    }

    /// Returns the properties associated w/ this object,
    /// 
    pub fn properties(&self) -> &Properties {
        match self {
            Object::Block(d) |
            Object::Root(d) |
            Object::Extension(d) => d.properties,
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
            _ => false
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
}

/// Object data format,
///
pub type ObjectFormat<'a> = (
    &'a ReadStorage<'a, Identifier>,
    &'a ReadStorage<'a, Properties>,
    MaybeJoin<&'a ReadStorage<'a, Block>>,
    MaybeJoin<&'a ReadStorage<'a, Root>>,
);

impl<'a> Load for Object<'a> {
    type Layout = ObjectFormat<'a>;

    fn load((identifier, properties, block, root): <Self::Layout as Join>::Type) -> Self {
        let object_data = ObjectData {
            identifier,
            properties,
            block,
            root,
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
        )
    }
}

impl<'a> AsRef<Entities<'a>> for Compiled<'a> {
    fn as_ref(&self) -> &Entities<'a> {
        &self.entities
    }
}
