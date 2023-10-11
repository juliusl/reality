use std::sync::Arc;

use futures_util::Stream;

use async_stream::stream;
use crate::{StorageTarget, ParsedAttributes, ResourceKey, Attribute};

/// wrapper struct to unpack parsed resources constructed by a project,
/// 
pub struct Node<S: StorageTarget + Send + Sync + 'static>(Arc<tokio::sync::RwLock<S>>);

impl<S: StorageTarget + Send + Sync + 'static> Node<S> {
    /// Returns a stream of attributes,
    /// 
    pub async fn stream_attributes(&self) -> impl Stream<Item = ResourceKey<Attribute>> + '_ {
        stream! {
            if let Some(parsed) = self.0.read().await.resource::<ParsedAttributes>(None) {
                for p in parsed.iter() {
                    yield *p;
                }
            }
        }
    }
}