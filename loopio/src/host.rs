use bytes::Bytes;
use reality::prelude::runir::prelude::Repr;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;

use reality::prelude::*;

use crate::prelude::Action;
use crate::prelude::ActionExt;
use crate::prelude::Address;
use crate::prelude::Ext;

/// A Host contains a broadly shared storage context,
///
#[derive(Reality, Default, Clone)]
#[plugin_def(call = debug)]
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
    #[reality(vec_of=Event)]
    pub event: Vec<Event>,
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
    let mut init = Remote.create::<Host>(tc).await;
    init.bind(tc.clone());

    for a in init.action.iter() {
        eprintln!("# Action -- {}", a.value().unwrap());

        if let Some(repr) = a.property.and_then(|p| p.repr()) {
            eprintln!("{:#}", repr);
        }
    }

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
            .field("event", &self.event)
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

/// Plugin for managing state for a shared event defined on a Host,
///
#[derive(
    Reality, Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
#[reality(call=on_event, plugin)]
pub struct Event {
    /// Name of this event,
    ///
    /// Decorations are passed from the host definition.
    ///
    #[reality(derive_fromstr)]
    pub name: String,
    /// Current state of this event,
    ///
    #[serde(skip)]
    #[reality(ignore)]
    pub data: Bytes,
    #[serde(skip)]
    #[reality(ignore)]
    pub repr: Repr,
}

async fn on_event(tc: &mut ThunkContext) -> anyhow::Result<()> {
    use tracing::debug;

    let init = tc.as_remote_plugin::<Event>().await;

    debug!(name = init.name);

    Ok(())
}

#[tokio::test]
#[tracing_test::traced_test]
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
    
<start/loopio.std.io.println>                   Hello World a
|# listen =     op_b_complete

+ .operation b
<loopio.std.io.println>                         Hello World b
|# notify =     op_b_complete

+ .operation c
<start/loopio.std.io.println>                   Hello World c
|# notify = test_cond

+ .operation d
<loopio.std.io.println>                         Hello World d

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

# -- Demo host
# -- Placeholder text 
+ .host demo
: .start        test

# -- Example of setting up a notifier
: .action               c/start/loopio.std.io.println
|# help     =           Example of adding help documentation
|# notify   =           ob_b_complete

# -- Example of wiring up a listener
: .action               a/start/loopio.std.io.println
|# help     =           Example of adding help documentation
|# listen   =           ob_b_complete

# -- Example of an event
: .event                op_b_complete
|# description  =       Example of an event that can be listened to
```
    "#,
    );

    let engine = crate::engine::Engine::builder().build();
    let engine = engine.compile(workspace).await.unwrap();
    // eprintln!("{:#?}", engine);

    let (eh, _) = engine.default_startup().await.unwrap();

    // Example - getting a virtual bus for an event created by host
    let mut vbus = eh.event_vbus("demo", "op_b_complete").await.unwrap();

    // Example - writing to an "event" created by host
    let mut txbus = vbus.clone();
    tokio::spawn(async move {
        // Example - transmit a change from another thread
        let transmit = txbus.transmit::<Event>().await;
        transmit.write_to_virtual(|r| {
            r.virtual_mut().owner.send_if_modified(|o| {
                o.data = Bytes::from_static(b"hello world");
                false
            });
            r.virtual_mut().name.commit()
        });
    });

    // Example - waiting for an "event" created by host
    let _event = vbus.wait_for::<Event>().await;
    let mut port = _event.select(|e| &e.virtual_ref().name);
    let mut port = futures_util::StreamExt::boxed(&mut port);
    if let Some((next, event)) = futures_util::StreamExt::next(&mut port).await {
        eprintln!("got next - {:#x?}", event);
        assert!(next.is_committed());
        assert_eq!(b"hello world", &event.data[..]);
    }

    ()
}
