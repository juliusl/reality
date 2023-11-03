use std::fmt::Debug;

use reality::prelude::*;

use crate::engine::EngineHandle;

/// A Host contains a broadly shared storage context,
///
#[derive(Reality, Default, Clone)]
pub struct Host {
    /// Name for this host,
    ///
    #[reality(derive_fromstr)]
    pub name: String,
    /// (unused) Tag for this host,
    ///
    #[reality(ignore)]
    pub _tag: Option<String>,
    /// Host storage provided by this host,
    ///
    #[reality(ignore)]
    pub host_storage: Option<AsyncStorageTarget<Shared>>,
    /// Engine handle,
    /// 
    #[reality(ignore)]
    pub handle: Option<EngineHandle>,
    /// Name of the action that "starts" this host,
    /// 
    #[reality(option_of=Tagged<String>)]
    start: Option<Tagged<String>>,
}

impl Host {
    /// Bind this host to a storage target,
    /// 
    pub fn bind(mut self, storage: AsyncStorageTarget<Shared>) -> Self {
        self.host_storage = Some(storage);
        self
    }

    /// Starts this host,
    /// 
    pub async fn start(&self) -> anyhow::Result<ThunkContext> {
        if let Some(engine) = self.handle.clone() {
            if let Some(start) = self.start.as_ref() {
                let address = match (start.tag(), start.value()) {
                    (None, Some(name)) => {
                        name.to_string()
                    }
                    (Some(tag), Some(name)) => {
                        format!("{name}#{tag}")
                    }
                    _ => {
                        unreachable!()
                    }
                };
    
                if let Some(seq) = engine.sequences.get(&address) {
                    seq.clone().await
                } else if let Some(mut op) = engine.operations.get(&address).cloned() {
                    if let Some(context) = op.context_mut() {
                        context.reset();
                    }
                    op.execute().await
                } else {
                    Err(anyhow::anyhow!("Start action cannot be found"))
                }
            } else {
                Err(anyhow::anyhow!("Start action is not set"))
            }
        } else {
            Err(anyhow::anyhow!("Host does not have an engine handle"))
        }
    }
}

impl Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("name", &self.name)
            .field("_tag", &self._tag)
            .field("handle", &self.handle)
            .field("start", &self.start)
            .finish()
    }
}
