use std::sync::Arc;

use futures_util::Stream;

use crate::prelude::*;
use crate::ParsedNode;
use async_stream::stream;

/// wrapper struct to unpack parsed resources constructed by a project,
///
pub struct Node<S: StorageTarget + Send + Sync + 'static>(pub Arc<tokio::sync::RwLock<S>>);

impl<S: StorageTarget + Send + Sync + 'static> From<Arc<tokio::sync::RwLock<S>>> for Node<S> {
    fn from(value: Arc<tokio::sync::RwLock<S>>) -> Self {
        Node(value)
    }
}

impl<S: StorageTarget + ToOwned<Owned = S> + Send + Sync + 'static> Node<S> {
    /// Returns a stream of attributes,
    ///
    pub fn stream_attributes(&self) -> impl Stream<Item = ResourceKey<Attribute>> + '_ {
        stream! {
            let parsed = self.0.latest().await.current_resource::<ParsedNode>(ResourceKey::root());
            if let Some(parsed) =  parsed {
                yield parsed.node;

                for p in parsed.parsed() {
                    yield p;
                }
            }
        }
    }
}

impl<S: StorageTarget + ToOwned<Owned = S> + Send + Sync + 'static> AsyncStorageTarget<S> {
    /// Returns a stream of attributes,
    ///
    pub fn stream_attributes(&self) -> impl Stream<Item = ResourceKey<Attribute>> + '_ {
        stream! {
            let parsed = self.storage.latest().await.current_resource::<ParsedNode>(ResourceKey::root());
            if let Some(parsed) =  parsed {
                yield parsed.node;

                for p in parsed.parsed() {
                    yield p;
                }
            }
        }
    }
}

impl Shared {
    /// Returns a stream of attributes,
    ///
    pub fn stream_attributes(&self) -> impl Stream<Item = ResourceKey<Attribute>> + '_ {
        stream! {
            let parsed = self.current_resource::<ParsedNode>(ResourceKey::root());
            if let Some(parsed) =  parsed {
                // yield parsed.node;

                for p in parsed.parsed() {
                    yield p;
                }
            }
        }
    }
}
