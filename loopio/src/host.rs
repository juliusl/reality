use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::Notify;

use reality::prelude::*;

use crate::prelude::Action;
use crate::prelude::Address;
use crate::prelude::Ext;

/// A Host contains a broadly shared storage context,
///
#[derive(Reality, Default, Clone)]
#[reality(call = debug, plugin)]
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
    /// List of actions to register w/ this host,
    ///
    #[reality(vec_of=Decorated<Address>)]
    pub action: Vec<Decorated<Address>>,
    /// Name of the action that "starts" this host,
    ///
    #[reality(option_of=Decorated<String>)]
    start: Option<Decorated<String>>,
    /// Set of conditions,
    ///
    #[reality(set_of=Decorated<HostCondition>)]
    condition: BTreeSet<Decorated<HostCondition>>,
    /// Set of events registered on this host,
    ///
    #[reality(set_of=Decorated<String>)]
    event: BTreeSet<Decorated<String>>,
    /// Binding to an engine,
    ///
    #[reality(ignore)]
    binding: Option<ThunkContext>,
    /// Node resource key,
    ///
    #[reality(ignore)]
    node: ResourceKey<reality::attributes::Node>,
    /// Plugin resource key,
    ///
    #[reality(ignore)]
    plugin: ResourceKey<reality::attributes::Attribute>,
}

async fn debug(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let mut init = Interactive.create::<Host>(tc).await;
    init.bind(tc.clone());
    // eprintln!("{:#?}", tc.decoration);

    // if let Some(block) = tc.parsed_block().await {
    //     if let Some(node) = block.nodes.get(&init.node) {
    //         eprintln!("{:#?}", node);
    //         if let Some(docs) = node.doc_headers.get(&init.node.transmute()) {
    //             for d in docs {
    //                 eprintln!("{d}");
    //             }
    //         }
    //     }
    // }

    for a in init.action.iter() {
        eprintln!("# Action -- {}", a.value().unwrap());
        if let Some(props) = a.decorations().map(|d| d.props()) {
            eprintln!("{:#?}", props);
        }
    }

    let block = tc.parsed_block().await.expect("should have parsed block");
    eprintln!("# Paths");
    for (p, _) in block.paths.iter() {
        eprintln!(" - {p}");
    }
    eprintln!();

    eprintln!("# Resource paths");
    for (p, _) in block.resource_paths.iter() {
        eprintln!(" - {p}");
    }
    eprintln!();

    if init.start.is_some() {
        eprintln!("Start found.");
        init.start().await?;
    } else {
        eprintln!("No start found.");
    }

    Ok(())
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
        if let Some(engine) = self.binding.as_ref() {
            let engine = engine
                .engine_handle()
                .await
                .expect("should be bound to an engine handle");

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
}

impl Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("name", &self.name)
            .field("_tag", &self._tag)
            .field("start", &self.start)
            .field("action", &self.action)
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

    fn bind_node(&mut self, node: ResourceKey<reality::attributes::Node>) {
        self.node = node;
    }

    fn bind_plugin(&mut self, plugin: ResourceKey<reality::attributes::Attribute>) {
        self.plugin = plugin;
    }

    fn node_rk(&self) -> ResourceKey<reality::attributes::Node> {
        self.node
    }

    fn plugin_rk(&self) -> Option<ResourceKey<reality::attributes::Attribute>> {
        Some(self.plugin)
    }
}

/// A condition specified on the host,
///
#[derive(Serialize, Deserialize, Debug)]
pub struct HostCondition {
    /// Name of the condition
    ///
    name: String,
    /// Last active unix timestamp of this condition,
    ///
    last_active: (AtomicU64, AtomicU64),
    /// Notification handle,
    ///
    #[serde(skip)]
    notify: Arc<Notify>,
}

impl HostCondition {
    pub fn new(name: impl Into<String>) -> HostCondition {
        HostCondition { name: name.into() , last_active: (AtomicU64::new(0), AtomicU64::new(0)), notify: Arc::new(Notify::new()) }
    }

    /// Notify observers of this condition,
    ///
    pub fn notify(&self) {
        let HostCondition { notify, .. } = self.clone();
        let (lo, hi) = uuid::Uuid::from_u128(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis())
            .as_u64_pair();

        let (_lo, _hi) = &self.last_active;
        let rlo = _lo.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |last| { 
            if last != lo {
                Some(lo)
            } else {
                None
            }
        });
        let rhi = _hi.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |last| {
            if last != hi {
                Some(hi)
            } else {
                None
            }
        });

        match (rlo, rhi) {
            (Ok(_l), Ok(_r)) |
            (Ok(_l), Err(_r)) |
            (Err(_l), Ok(_r))|
            (Err(_l), Err(_r)) => {
                notify.notify_waiters();
            }
        }
    }

    /// Observe this condition,
    ///
    /// returns when the condition has completed
    ///
    pub fn listen(&self) -> Arc<Notify> {
        let HostCondition { notify, .. } = self.clone();
        notify.clone()
    }

    /// Pings any listeners,
    /// 
    pub fn ping(&self) -> Option<u128> {
        None
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
        Self {
            name: self.name.clone(),
            notify: self.notify.clone(),
            last_active: (AtomicU64::new(0), AtomicU64::new(0)),
        }
    }
}

impl Eq for HostCondition {}

impl FromStr for HostCondition {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(HostCondition {
            name: s.to_string(),
            notify: Arc::new(Notify::new()),
            last_active: (AtomicU64::new(0), AtomicU64::new(0)),
        })
    }
}

#[tokio::test]
async fn test_host_condition() {
    let condition = HostCondition::new("test_host_condition");
    let cond = condition.clone();
    let notified_a = cond.listen();
    let notified_a = notified_a.notified();
    let notified_b = cond.listen();
    let notified_b = notified_b.notified();

    tokio::spawn(async move { 
        condition.notify();
    });
    
    notified_a.await;
    notified_b.await;
    ()
}

#[tokio::test]
async fn test_host() {
    let mut workspace = Workspace::new();
    workspace.add_buffer(
        "demo.md",
        r#"
    ```runmd
    # -- Example operation that listens for the completion of another
    # -- 
    + .operation a
    |# test = test
    
    <loopio.std.io.println>                 Hello World a
    |# listen =     op_b_complete

    + .operation b
    <loopio.std.io.println>                 Hello World b
    |# notify =     op_b_complete

    + .operation c
    <start/loopio.std.io.println>           Hello World c

    + .operation d
    <loopio.std.io.println>                 Hello World d

    # -- Test sequence decorations
    + .sequence test
    |# name = Test sequence
    
    # -- Operations on a step execute all at once
    :  .step a, b, c

    # -- If kind is set to once, this row only executes once if the sequence loops
    : .step b, d,
    |# kind = once

    # -- If this were set to true, then the sequence would automatically loop
    : .loop false

    + .host demo
    : .start        test
    
    : .action       c/start/loopio.std.io.println
    |# help  =      Example of adding help documentation
    |# route =      start_c

    : .action       b

    : .event        op_b_complete
    |# description = Example of an event that can be listened to

    ```
    "#,
    );

    let engine = crate::engine::Engine::builder().build();
    let engine = engine.compile(workspace).await;
    let block = engine.block().unwrap();
    let eh = engine.engine_handle();
    let deck = crate::deck::Deck::from(block);
    eprintln!("{:#?}", deck);
    let _e = engine.spawn(|_, p| Some(p));
    if let Ok(hosted_resource) = eh.hosted_resource("demo://").await {
        eprintln!("Found hosted resource - {}", hosted_resource.address());

        hosted_resource.spawn().unwrap().await.unwrap().unwrap();
    }

    ()
}
