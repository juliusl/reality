use std::sync::Arc;

use futures_util::Stream;

use crate::ParsedAttributes;
use crate::prelude::*;
use async_stream::stream;

/// wrapper struct to unpack parsed resources constructed by a project,
///
pub struct Node<S: StorageTarget + Send + Sync + 'static>(pub Arc<tokio::sync::RwLock<S>>);

impl<S: StorageTarget + ToOwned<Owned = S> + Send + Sync + 'static> Node<S> {
    /// Returns a stream of attributes,
    ///
    pub fn stream_attributes(&self) -> impl Stream<Item = ResourceKey<Attribute>> + '_ {
        stream! {
            let parsed = self.0.latest().await.current_resource::<ParsedAttributes>(None);
            if let Some(parsed) =  parsed {
                for p in parsed.iter() {
                    yield *p;
                }
            }
        }
    }
}
