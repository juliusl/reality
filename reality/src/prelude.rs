pub use crate::derive::AttributeType;
pub use crate::derive::Reality;
pub use crate::derive::RealityEnum;
pub use crate::derive::RealityTest;
pub use crate::project::Program;
pub use crate::AsyncStorageTarget;
pub use crate::Attribute;
pub use crate::AttributeParser;
pub use crate::AttributeType;
pub use crate::BlockObject;
pub use crate::CacheExt;
pub use crate::CurrentDir;
pub use crate::Decorated;
pub use crate::Delimitted;
pub use crate::Dir;
pub use crate::Dispatcher;
pub use crate::EmptyWorkspace;
pub use crate::Field;
pub use crate::FieldMut;
pub use crate::FieldOwned;
pub use crate::FieldPacket;
pub use crate::FieldPacketType;
pub use crate::FieldRef;
pub use crate::FieldRefController;
pub use crate::FieldVTable;
pub use crate::Frame;
pub use crate::FrameListener;
pub use crate::FrameUpdates;
pub use crate::HostedResource;
pub use crate::NewFn;
pub use crate::Node;
pub use crate::OnParseField;
pub use crate::OnReadField;
pub use crate::OnWriteField;
pub use crate::Pack;
pub use crate::PacketRouter;
pub use crate::PacketRoutes;
pub use crate::ParsableField;
pub use crate::ParsedBlock;
pub use crate::ParsedNode;
pub use crate::Project;
pub use crate::Property;
pub use crate::RegisterWith;
pub use crate::ResourceKey;
pub use crate::ResourceKeyHashBuilder;
pub use crate::SetField;
pub use crate::SetIdentifiers;
pub use crate::Shared;
pub use crate::Source;
pub use crate::StorageTarget;
pub use crate::ToFrame;
pub use crate::Transform;
pub use crate::VisitVirtual;
pub use crate::VisitVirtualMut;
pub use crate::WireServer;
pub use crate::Workspace;
pub use crate::project::Package;

// pub use crate::SharedFile;

pub use crate::enable_virtual_dependencies;

/*
    Macros for working w/ a storage target
*/
pub use crate::borrow;
pub use crate::borrow_mut;
pub use crate::resource;
pub use crate::resource_mut;
pub use crate::resource_owned;
pub use crate::take;
pub use crate::task;
pub use crate::task_mut;

pub use crate::thunk::*;

pub use std::str::FromStr;

pub use async_trait::async_trait;

pub use runir;

/// Returns the latest value of a reference,
///
#[async_trait::async_trait]
pub trait Latest<T>
where
    T: ToOwned<Owned = T> + Send + Sync + 'static,
{
    /// Returns the latest value,
    ///
    async fn latest(&self) -> T;
}

#[async_trait::async_trait]
impl<T> Latest<T> for tokio::sync::RwLock<T>
where
    T: ToOwned<Owned = T> + Send + Sync + 'static,
{
    async fn latest(&self) -> T {
        self.read().await.to_owned()
    }
}
