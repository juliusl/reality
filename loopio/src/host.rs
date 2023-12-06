
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
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
#[reality(call = debug, plugin, unload=on_unload)]
pub struct Host {
    /// Name for this host,
    ///
    #[reality(derive_fromstr)]
    pub name: Decorated<String>,
    /// List of actions to register w/ this host,
    ///
    #[reality(vec_of=Decorated<Address>)]
    pub action: Vec<Decorated<Address>>,
    /// Name of the action that "starts" this host,
    ///
    #[reality(option_of=Decorated<Address>)]
    pub start: Option<Decorated<Address>>,
    /// List of events managed by this host,
    ///
    #[reality(vec_of=Decorated<HostEvent>)]
    pub event: Vec<Decorated<HostEvent>>,
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

async fn on_unload<S: StorageTarget>(storage: AsyncStorageTarget<S>, _: Option<ResourceKey<Attribute>>) {
    let storage = storage.storage.read().await;

    eprintln!("on unload being called");
    if let Some(init) = storage.resource::<Host>(ResourceKey::root()) {
        let _v_proxy = VirtualHost::new(init.to_owned());

        _v_proxy.name.view_value(|v| {
            eprintln!("{:?}", v);
        });
    } else {
        eprintln!("did not find initialized resource");
    }
    ()
}

async fn debug(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let mut init = Remote.create::<Host>(tc).await;
    init.bind(tc.clone());

    if let Some(docs) = tc.decoration.as_ref().and_then(|d| d.doc_headers.as_ref()) {
        for d in docs {
            eprintln!("{}", d);
        }
    }

    let block = tc.parsed_block().await.expect("should have parsed block");

    if let Some(node) = block.nodes.get(&init.node) {
        for (_, d) in node.doc_headers.iter() {
            d.iter().for_each(|e| eprintln!("{}", e));
        }
    }

    for a in init.action.iter() {
        eprintln!("# Action -- {}", a.value().unwrap());
        if let Some(props) = a.decorations().map(|d| d.props()) {
            eprintln!("{:#?}", props);
        }
    }

    eprintln!("# Paths");
    for (p, _) in block.paths.iter() {
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
    /// Returns true if a condition has been signaled,
    ///
    /// Returns false if this condition is not registered w/ this host.
    ///
    pub fn set_condition(&self, _condition: impl AsRef<str>) -> bool {
        false
    }

    /// Starts this host,
    ///
    pub async fn start(&self) -> anyhow::Result<ThunkContext> {
        if let Some(engine) = self.binding.as_ref() {
            let engine = engine
                .engine_handle()
                .await
                .expect("should be bound to an engine handle");

            if let Some(start) = self.start.as_ref().and_then(|s| s.value()) {
                let mut resource = engine.hosted_resource(start.to_string()).await?;

                resource.context_mut().write_cache(self.event.clone());

                resource.spawn_call().await
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
            .field("start", &self.start)
            .field("action", &self.action)
            .finish()
    }
}

impl SetIdentifiers for Host {
    fn set_identifiers(&mut self, name: &str, tag: Option<&String>) {
        self.name.value = Some(name.to_string());
        self.name.tag = tag.cloned();
    }
}

impl Action for Host {
    #[inline]
    fn address(&self) -> String {
        format!(
            "{}://",
            self.name.value().unwrap_or(&String::from("engine"))
        )
    }

    #[inline]
    fn context(&self) -> &ThunkContext {
        self.binding.as_ref().expect("should be bound to an engine")
    }

    #[inline]
    fn context_mut(&mut self) -> &mut ThunkContext {
        self.binding.as_mut().expect("should be bound to an engine")
    }

    #[inline]
    fn bind(&mut self, context: ThunkContext) {
        self.binding = Some(context);
    }

    #[inline]
    fn bind_node(&mut self, node: ResourceKey<reality::attributes::Node>) {
        self.node = node;
    }

    #[inline]
    fn bind_plugin(&mut self, plugin: ResourceKey<reality::attributes::Attribute>) {
        self.plugin = plugin;
    }

    #[inline]
    fn node_rk(&self) -> ResourceKey<reality::attributes::Node> {
        self.node
    }

    #[inline]
    fn plugin_rk(&self) -> ResourceKey<reality::attributes::Attribute> {
        self.plugin
    }
}

/// An event managed by the host,
///
#[derive(Serialize, Deserialize, Debug)]
pub struct HostEvent {
    /// Name of the event
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

impl HostEvent {
    /// Creates a new host condition,
    ///
    pub fn new(name: impl Into<String>) -> HostEvent {
        HostEvent {
            name: name.into(),
            last_active: (AtomicU64::new(0), AtomicU64::new(0)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Notify observers of this condition,
    ///
    pub fn notify(&self) {
        let HostEvent { notify, .. } = self.clone();
        let (lo, hi) = uuid::Uuid::from_u128(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .as_u64_pair();

        let (_lo, _hi) = &self.last_active;
        let rlo = _lo.fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |last| {
                if last != lo {
                    Some(lo)
                } else {
                    None
                }
            },
        );
        let rhi = _hi.fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |last| {
                if last != hi {
                    Some(hi)
                } else {
                    None
                }
            },
        );

        match (rlo, rhi) {
            (Ok(_l), Ok(_r)) | (Ok(_l), Err(_r)) | (Err(_l), Ok(_r)) | (Err(_l), Err(_r)) => {
                notify.notify_waiters();
            }
        }
    }

    /// Observe this condition,
    ///
    /// returns when the condition has completed
    ///
    pub fn listen(&self) -> Arc<Notify> {
        let HostEvent { notify, .. } = self.clone();
        notify.clone()
    }

    /// Pings any listeners,
    ///
    pub fn ping(&self) -> Option<u128> {
        None
    }
}

impl Ord for HostEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for HostEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HostEvent {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Clone for HostEvent {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            notify: self.notify.clone(),
            last_active: (AtomicU64::new(0), AtomicU64::new(0)),
        }
    }
}

impl Eq for HostEvent {}

impl FromStr for HostEvent {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(HostEvent {
            name: s.to_string(),
            notify: Arc::new(Notify::new()),
            last_active: (AtomicU64::new(0), AtomicU64::new(0)),
        })
    }
}

#[tokio::test]
async fn test_host_condition() {
    let condition = HostEvent::new("test_host_condition");
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
    use crate::prelude::VirtualBusExt;
    use futures_util::pin_mut;

    let mut workspace = Workspace::new();
    workspace.add_buffer(
        "demo.md",
        r#"
    ```runmd
    # -- Example operation that listens for the completion of another
    # -- 
    + .operation a
    |# test = test
    
    <start/loopio.std.io.println>                 Hello World a
    |# listen =     op_b_complete

    + .operation b
    <loopio.std.io.println>                 Hello World b
    |# notify =     op_b_complete

    + .operation c
    <start/loopio.std.io.println>           Hello World c
    |# notify = test_cond

    + .operation d
    <loopio.std.io.println>                 Hello World d

    # -- Test sequence decorations
    + .sequence test
    |# name = Test sequence
    
    # -- Operations on a step execute all at once
    :  .step    c/start/loopio.std.io.println,
    |           a/start/loopio.std.io.println

    :  .step    b, d
    |# kind     =   once

    # -- If this were set to true, then the sequence would automatically loop
    : .loop false

    # -- # Demo host
    # -- Placeholder text 
    + .host demo
    : .start        test

    # -- # Example of setting up a notifier
    : .action               c/start/loopio.std.io.println
    |# help     =           Example of adding help documentation
    |# notify   =           ob_b_complete

    # -- # Example of wiring up a listener
    : .action               a/start/loopio.std.io.println
    |# help     =           Example of adding help documentation
    |# listen   =           ob_b_complete

    # -- # Example of an event
    : .event                op_b_complete
    |# description  =       Example of an event that can be listened to
    
    
    ```
    "#,
    );

    let engine = crate::engine::Engine::builder().build();
    let engine = engine.compile(workspace).await;
    // eprintln!("{:#?}", engine);

    let block = engine.block().unwrap();
    let eh = engine.engine_handle();
    let _deck = crate::deck::Deck::from(block);
    // eprintln!("{:#?}", deck);
    let _e = engine.spawn(|_, p| {
        eprintln!("{:?}", p);
        Some(p)
    });
    if let Ok(hosted_resource) = eh.hosted_resource("demo://").await {
        eprintln!("Found hosted resource - {}", hosted_resource.address());
        hosted_resource.spawn().unwrap().await.unwrap().unwrap();
        hosted_resource.spawn().unwrap().await.unwrap().unwrap();

        if let Ok(Some(tc)) = hosted_resource.context().enable_virtual().await {
            let node = tc.node().await;
            let virtual_host = node
                .current_resource::<Arc<tokio::sync::watch::Sender<VirtualHost>>>(
                    tc.attribute.transmute(),
                )
                .expect("should be enabled")
                .clone();
            drop(node);
            let _eh = eh.clone();

            // Receiver
            // There are two levels of listening
            // In this case it is the first level
            // This example simulates what would happen when a plugin is called that 
            // will listen for events.

            let listener = tokio::spawn(async move {
                if let Ok(hosted_resource) = _eh
                    .hosted_resource("demo://a/start/loopio.std.io.println")
                    .await
                {
                    eprintln!("{:#?}", hosted_resource.decoration);

                    let mut stream = hosted_resource
                        .context()
                        .virtual_bus("demo://".parse::<Address>().unwrap())
                        .await
                        .wait_for::<Host>()
                        .await
                        .select(|h| &h.name)
                        .filter(|h| h.is_committed());

                    let stream = &mut stream;

                    pin_mut!(stream);

                    eprintln!("waiting for committed");
                    let name_committed = futures_util::StreamExt::next(&mut stream).await;

                    if let Some((field_changed, _latest)) = name_committed {
                        field_changed.view_value(|v| {
                            eprintln!("!!!Name was committed!!! {:?}", v);
                            assert_eq!(v.value.as_ref().unwrap().as_str(), "demo2");
                        });
                    }

                    eprintln!("Found hosted resource - {}", hosted_resource.address());
                    hosted_resource.spawn().unwrap().await.unwrap().unwrap();
                    hosted_resource.spawn().unwrap().await.unwrap().unwrap();
                }
                eprintln!("Host was modified");
            });

            // let eh = eh.clone();
            // let listener_2 = tokio::spawn(async move {
            //     if let Ok(hosted_resource) = eh
            //         .hosted_resource("demo://a/start/loopio.std.io.println")
            //         .await
            //     {
            //         let mut rx = hosted_resource
            //             .context()
            //             .virtual_bus("demo://".parse::<Address>().unwrap())
            //             .await
            //             .wait_for::<Host>()
            //             .await
            //             .subscribe_raw();

            //         rx.changed().await.unwrap();

            //         eprintln!("Got host change from raw subscription");
            //         assert_eq!(rx.borrow().clone().name.value().unwrap().as_str(), "demo2");
            //     }
            // });

            // Transmitter
            let tx = virtual_host.clone().borrow().name.clone().start_tx();
            if let Ok(_) = tx.next(|host| Ok(host)).finish() {
                // Get a reference to the bus
                let mut commit = hosted_resource
                    .context()
                    .virtual_bus("demo://".parse::<Address>().unwrap())
                    .await;

                eprintln!("Waiting for sync");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                // Create a new commit channel
                let commit = commit.transmit::<Host>().await;

                // Write a change to the virtual reference
                // TODO -- improve api ergo, deref mut is to owned so need to figure out how to mutate inner
                // field ref
                commit.write_to_virtual(|virt: &mut VirtualHost| {
                    virt.name.edit_value(|f, v| {
                        assert_eq!(f, "name");
                        v.value = Some(String::from("demo2"));
                        true
                    });
                    assert!(virt.name.commit());
                    assert!(!virt.name.commit());
                    eprintln!("Committed --");
                    true
                });

                listener.await.unwrap();
                // listener_2.await.unwrap();

                let mut node = hosted_resource.context().node.storage.write().await;
                node.put_resource(virtual_host, hosted_resource.plugin_rk().transmute());
            } else {
                panic!("Expecting the listener to be released");
            }
        }
    }

    // if let Ok(mut h) = eh.hosted_resource("engine://test").await {
    //     let seq =
    //         crate::action::ActionExt::as_local_plugin::<crate::prelude::Sequence>(&mut h).await;
    //     eprintln!("{:?}", seq.context().decoration);
    // }

    // if let Ok(hosted_resource) = eh
    //     .hosted_resource("demo://a/start/loopio.std.io.println")
    //     .await
    // {
    //     eprintln!("{:#?}", hosted_resource.decoration);
    // }
    ()
}
