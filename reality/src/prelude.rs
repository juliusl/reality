pub use crate::derive::BlockObjectType;
pub use crate::derive::AttributeType;
pub use crate::AttributeType;
pub use crate::StorageTarget;
pub use crate::AttributeParser;
pub use crate::BlockObject;
pub use crate::AsyncStorageTarget;
pub use crate::OnParseField;
pub use crate::Tagged;
pub use crate::Project;
pub use crate::Shared;
pub use crate::Extension;
pub use crate::ExtensionController;
pub use crate::ExtensionPlugin;
pub use crate::ResourceKey;
pub use crate::ResourceKeyHashBuilder;


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

pub use std::str::FromStr;