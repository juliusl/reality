pub use crate::derive::Reality;
pub use crate::derive::AttributeType;
pub use crate::derive::RealityEnum;
pub use crate::derive::RealityTest;
pub use crate::Attribute;
pub use crate::AttributeType;
pub use crate::ParsedAttributes;
pub use crate::Properties;
pub use crate::Comments;
pub use crate::StorageTarget;
pub use crate::AttributeParser;
pub use crate::BlockObject;
pub use crate::AsyncStorageTarget;
pub use crate::Dispatcher;
pub use crate::OnParseField;
pub use crate::Property;
pub use crate::Delimitted;
pub use crate::Decoration;
pub use crate::Decorated;
pub use crate::Project;
pub use crate::Source;
pub use crate::Workspace;
pub use crate::CurrentDir;
pub use crate::Shared;
pub use crate::Transform;
pub use crate::ResourceKey;
pub use crate::ResourceKeyHashBuilder;
pub use crate::Visit;
pub use crate::VisitMut;
pub use crate::Frame;
pub use crate::FrameUpdates;
pub use crate::ToFrame;
pub use crate::FieldPacket;
pub use crate::FieldPacketType;
pub use crate::Field;
pub use crate::FieldMut;
pub use crate::FieldOwned;
pub use crate::SetField;
pub use crate::RegisterWith;

/*
    Macros for working w/ a storage target
*/
pub use crate::take;
pub use crate::resource;
pub use crate::resource_mut;
pub use crate::resource_owned;
pub use crate::borrow;
pub use crate::borrow_mut;
pub use crate::task;
pub use crate::task_mut;

pub use crate::thunk::*;

pub use std::str::FromStr;

pub use async_trait::async_trait;

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
