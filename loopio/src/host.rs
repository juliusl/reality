use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::Notify;

use reality::prelude::*;

use crate::engine::EngineHandle;

/// A condition specified on the host,
/// 
pub struct HostCondition(String, Arc<Notify>);

impl HostCondition {
    /// Notify observers of this condition,
    /// 
    pub fn notify(&self) {
        let HostCondition(_, notify) = self.clone(); 
        
        notify.notify_waiters();
    }

    /// Observe this condition, 
    /// 
    /// returns when the condition has completed
    /// 
    pub async fn listen(&self) {
        let HostCondition(_, notify) = self.clone(); 
        
        notify.notified().await;
    }
}

impl Ord for HostCondition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for HostCondition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HostCondition {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Clone for HostCondition {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl Eq for HostCondition {}

impl FromStr for HostCondition {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(HostCondition(s.to_string(), Arc::new(Notify::new())))
    }
}

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
    /// Vector of child hosts,
    /// 
    /// Only used by the default host,
    /// 
    #[reality(ignore)]
    pub(crate) children: BTreeMap<String, Host>,
    /// Name of the action that "starts" this host,
    ///
    #[reality(option_of=Tagged<String>)]
    start: Option<Tagged<String>>,
    /// Map of conditions,
    ///
    #[reality(set_of=HostCondition)]
    condition: BTreeSet<HostCondition>,
}

impl Host {
    /// Bind this host to a storage target,
    ///
    pub fn bind(mut self, storage: AsyncStorageTarget<Shared>) -> Self {
        self.host_storage = Some(storage);
        self
    }

    /// Returns true if a condition has been signaled,
    /// 
    /// Returns false if this condition is not registered w/ this host.
    ///
    pub fn set_condition(&self, condition: impl AsRef<str>) -> bool {
        if let Some(condition) = self
            .condition
            .iter()
            .find(|c| c.0 == condition.as_ref())
            .map(|c| c.1.clone())
        {
            condition.clone().notify_waiters();
            true
        } else {
            false
        }
    }

    /// Starts this host,
    ///
    pub async fn start(&self) -> anyhow::Result<ThunkContext> {
        self.initialized_conditions().await;
        
        if let Some(engine) = self.handle.clone() {
            if let Some(start) = self.start.as_ref() {
                let address = match (start.tag(), start.value()) {
                    (None, Some(name)) => name.to_string(),
                    (Some(tag), Some(name)) => {
                        format!("{name}#{tag}")
                    }
                    _ => {
                        unreachable!()
                    }
                };

                engine.run(address).await
            } else {
                Err(anyhow::anyhow!("Start action is not set"))
            }
        } else {
            Err(anyhow::anyhow!("Host does not have an engine handle"))
        }
    }

    /// Initialize conditions on the host,
    /// 
    async fn initialized_conditions(&self) {
        if let Some(storage) = self.host_storage.as_ref().map(|h| h.storage.clone()) {
            let mut storage = storage.write().await;

            for condition in self.condition.iter() {
                storage.put_resource(
                    condition.clone(),
                    Some(ResourceKey::with_hash(&condition.0)),
                );
            }
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
