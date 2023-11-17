use reality::SetIdentifiers;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::Notify;

use reality::prelude::*;

use crate::address::Action;
use crate::prelude::Address;
use crate::prelude::Ext;

/// An address to listen to,
///
/// When the address has been activated, the condition configured will be notified.
///
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostListen {
    //// Addresss to listen to,
    ///
    address: String,
    /// Name of the condition to notify,
    ///
    condition: String,
}

impl FromStr for HostListen {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(HostListen {
            address: s.to_string(),
            condition: String::new(),
        })
    }
}

/// A condition specified on the host,
///
#[derive(Serialize, Deserialize)]
pub struct HostCondition {
    /// Name of the condition
    name: String,
    /// Notification handle,
    #[serde(skip)]
    notify: Arc<Notify>,
}

impl HostCondition {
    /// Notify observers of this condition,
    ///
    pub fn notify(&self) {
    let HostCondition { notify, ..} = self.clone();

        notify.notify_waiters();
    }

    /// Observe this condition,
    ///
    /// returns when the condition has completed
    ///
    pub async fn listen(&self) {
        let HostCondition { notify, ..} = self.clone();

        notify.notified().await;
    }
}

impl Ord for HostCondition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for HostCondition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HostCondition {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Clone for HostCondition {
    fn clone(&self) -> Self {
        Self { name: self.name.clone(), notify: self.notify.clone() }
    }
}

impl Eq for HostCondition {}

impl FromStr for HostCondition {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(HostCondition {
            name: s.to_string(),
            notify: Arc::new(Notify::new()),
        })
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
    /// Vector of child hosts,
    ///
    /// Only used by the default host,
    ///
    #[reality(ignore)]
    pub(crate) children: BTreeMap<String, Host>,
    /// Name of the action that "starts" this host,
    ///
    #[reality(option_of=Decorated<String>)]
    start: Option<Decorated<String>>,
    /// List of actions to register w/ this host,
    /// 
    #[reality(vec_of=Decorated<Address>)]
    pub action: Vec<Decorated<Address>>,
    /// Set of conditions,
    ///
    #[reality(set_of=Decorated<HostCondition>)]
    condition: BTreeSet<Decorated<HostCondition>>,
    /// Set of listeners,
    /// 
    #[reality(set_of=Decorated<HostListen>)]
    listen: BTreeSet<Decorated<HostListen>>,
    /// Binding to an engine,
    /// 
    #[reality(ignore)]
    binding: Option<ThunkContext>,
}

impl Host {
    /// Bind this host to a storage target,
    ///
    pub fn bind_storage(mut self, storage: AsyncStorageTarget<Shared>) -> Self {
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
            .filter_map(|c| c.value())
            .find(|c| c.name == condition.as_ref())
            .map(|c| c.notify.clone())
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

        if let Some(engine) = self.binding.as_ref() {
            let engine = engine.engine_handle().await.expect("should be bound to an engine handle");

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

            for condition in self.condition.iter().filter_map(|c| c.value()) {
                storage.put_resource(
                    condition.clone(),
                    Some(ResourceKey::with_hash(&condition.name)),
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
            .field("start", &self.start)
            .field("has_binding", &self.binding.is_some())
            .finish()
    }
}

impl SetIdentifiers for Host {
    fn set_identifiers(&mut self, name: &str, tag: Option<&String>) {
        self.name = name.to_string();
        self._tag = tag.cloned();
    }
}

impl Action for Host {
    fn address(&self) -> String {
        format!("{}://", self.name)
    }

    fn context(&self) -> &ThunkContext {
        self.binding.as_ref().expect("should be bound to an engine")
    }

    fn context_mut(&mut self) -> &mut ThunkContext {
        self.binding.as_mut().expect("should be bound to an engine")
    }

    fn bind(&mut self, context: ThunkContext) {
        self.binding = Some(context);
    }
}