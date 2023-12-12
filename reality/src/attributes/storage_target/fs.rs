// use std::{collections::HashMap, path::PathBuf, sync::Arc};

// use bytes::Bytes;
// use tokio::sync::RwLock;

// use crate::{Attribute, ResourceKey, Shared, StorageTarget};

// // use serde::Serialize;
// // use serde::de::DeserializeOwned;
// // use tokio::sync::RwLock;

// // use crate::{Shared, StorageTarget};

// /// Struct containing a cell which is a pointer to a shared resource,
// ///
// /// And a handle to memory owned by the cell.
// ///
// /// Data is loaded by a file source when/if available. By default, data
// /// is empty and does not allocate.
// ///
// pub struct FileCell {
//     /// Shared cell,
//     ///
//     cell: Arc<RwLock<Box<dyn Send + Sync + 'static>>>,
//     /// Data owned by cell,
//     ///
//     data: Bytes,
// }

// /// File-backed shared storage target,
// ///
// pub struct SharedFile {
//     /// Map of resources that are backed by a file-cell,
//     ///
//     resources: HashMap<ResourceKey<Attribute>, FileCell>,

//     /// Main storage,
//     /// 
//     storage: Shared,

//     /// Local-Working directory,
//     ///
//     /// A working directory should be able to be deleted and re-initialized. There
//     /// should be no guranttees made w/ data in the working directory.
//     ///
//     working: PathBuf,
//     /// Local-Config directory,
//     ///
//     /// A config directory must be stable. Any changes to the directory should be programmatic.
//     /// If a config directory does not exist, then it must be considered uninitialized.
//     ///
//     config: PathBuf,
// }

// impl StorageTarget for SharedFile {
//     type BorrowResource<'a, T: Send + Sync + 'static> =
//         <Shared as StorageTarget>::BorrowResource<'a, T>;

//     type BorrowMutResource<'a, T: Send + Sync + 'static> =
//         <Shared as StorageTarget>::BorrowMutResource<'a, T>;

//     type Namespace = <Shared as StorageTarget>::Namespace;

//     fn create_namespace() -> Self::Namespace {
//         Shared::default()
//     }

//     fn len(&self) -> usize {
//         self.resources.len()
//     }

//     fn remove_resource_at(&mut self, key: ResourceKey<Attribute>) -> bool {
//         self.storage.remove_resource_at(key)
//     }

//     fn maybe_put_resource<T: Send + Sync + 'static>(
//         &mut self,
//         _resource: T,
//         _resource_key: crate::StorageTargetKey<T>,
//     ) -> Self::BorrowMutResource<'_, T> {
//         self.storage.maybe_put_resource(_resource, _resource_key)
//     }

//     fn put_resource<T: Send + Sync + 'static>(
//         &mut self,
//         resource: T,
//         resource_key: crate::StorageTargetKey<T>,
//     ) {
//         self.storage.put_resource(resource, resource_key)
//     }

//     fn take_resource<T: Send + Sync + 'static>(
//         &mut self,
//         resource_key: crate::StorageTargetKey<T>,
//     ) -> Option<Box<T>> {
//         self.storage.take_resource(resource_key)
//     }

//     fn resource<'a: 'b, 'b, T: Send + Sync + 'static>(
//         &'a self,
//         resource_key: crate::StorageTargetKey<T>,
//     ) -> Option<Self::BorrowResource<'b, T>> {
//         self.storage.resource(resource_key)
//     }

//     fn resource_mut<'a: 'b, 'b, T: Send + Sync + 'static>(
//         &'a mut self,
//         resource_key: crate::StorageTargetKey<T>,
//     ) -> Option<Self::BorrowMutResource<'b, T>> {
//         self.storage.resource_mut(resource_key)
//     }
// }
