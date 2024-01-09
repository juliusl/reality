use std::collections::VecDeque;
use std::task::Poll;

use futures_util::FutureExt;
use reality::prelude::*;
use reality::SetIdentifiers;
use tokio::pin;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tracing::error;
use tracing::trace;

use crate::{ext::Ext, prelude::Action};

/// Struct containing steps of a sequence of operations,
///
#[derive(Reality, Default)]
#[reality(call = execute_sequence, plugin)]
pub struct Sequence {
    /// Name of this sequence,
    ///
    #[reality(ignore)]
    pub name: String,
    /// Tag of this sequence,
    ///
    #[reality(ignore)]
    pub tag: Option<String>,
    /// Steps that should be executed one-after the other,
    ///
    #[reality(vecdeq_of=Decorated<Delimitted<',', Step>>)]
    step: VecDeque<Decorated<Delimitted<',', Step>>>,
    /// Indicates the sequence should loop,
    ///
    #[reality(rename = "loop")]
    _loop: bool,
    /// Step list,
    ///
    #[reality(ignore)]
    _step_list: Option<StepList>,
    /// Current sequence being run,
    ///
    #[reality(ignore)]
    current: Option<JoinHandle<anyhow::Result<ThunkContext>>>,
    /// ThunkContext this sequence is bounded to,
    ///
    #[reality(ignore)]
    pub binding: Option<ThunkContext>,
    #[reality(ignore)]
    node: ResourceKey<reality::attributes::Node>,
    #[reality(ignore)]
    plugin: ResourceKey<reality::attributes::Attribute>,
}

async fn execute_sequence(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let mut seq = Remote.create::<Sequence>(tc).await;
    seq.bind(tc.clone());
    seq.context_mut().attribute = tc.attribute;

    //
    // A sequence tracks what it has already called w/ the StepList
    // When restoring the list any "once" steps are filtered out.
    //
    // In order to make changes that way, we need to pin the sequence before calling it, so that we can persist
    // the result afterwards.
    //
    pin!(seq);

    (&mut seq).await?;

    {
        let seq = seq.get_mut();
        tc.node()
            .await
            .lazy_put_resource(seq.clone(), tc.attribute.transmute());
    }

    tc.context_mut().process_node_updates().await;

    Ok(())
}

impl SetIdentifiers for Sequence {
    fn set_identifiers(&mut self, name: &str, tag: Option<&String>) {
        self.name = name.to_string();
        self.tag = tag.cloned();
    }
}

impl Clone for Sequence {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            tag: self.tag.clone(),
            step: self.step.clone(),
            _loop: self._loop,
            _step_list: self._step_list.clone(),
            binding: self.binding.clone(),
            current: None,
            node: self.node,
            plugin: self.plugin,
        }
    }
}

impl std::fmt::Debug for Sequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sequence")
            .field("name", &self.name)
            .field("tag", &self.tag)
            .field("step", &self.step)
            .field("_loop", &self._loop)
            .field("has_binding", &self.binding.is_some())
            .field("has_current", &self.current.is_some())
            .finish()
    }
}

impl Sequence {
    /// Returns a new empty sequence,
    ///
    pub fn new(name: impl Into<String>, tag: Option<String>) -> Self {
        Self {
            name: name.into(),
            tag,
            step: vec![].into(),
            binding: None,
            current: None,
            _loop: false,
            _step_list: None,
            plugin: ResourceKey::default(),
            node: ResourceKey::default(),
        }
    }

    /// Returns the next operation to run,
    ///
    /// If None is returned, it signals the end of the sequence.
    ///
    /// If _loop is true, after None is returned it will reset the cursor, such
    /// that next() will then return the beginning of the next sequence.
    ///
    pub fn next_step(&mut self) -> Option<Vec<Step>> {
        if let Some(steps) = self._step_list.as_mut() {
            let next = steps.next();
            if next.is_none() {
                self._step_list = Some(StepList(
                    self.step
                        .iter()
                        .filter(|s| {
                            s.property("kind")
                                .as_ref()
                                .filter(|k| k.as_str() == "once")
                                .is_none()
                        })
                        .cloned()
                        .collect(),
                ));

                if self._loop {
                    return self.next_step();
                }
            }
            next
        } else {
            self._step_list = Some(StepList(self.step.clone()));
            self.next_step()
        }
    }
}

impl std::future::Future for Sequence {
    type Output = anyhow::Result<ThunkContext>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        if self
            .binding
            .as_ref()
            .map(|b| b.cancellation.is_cancelled())
            .unwrap_or_default()
        {
            return Poll::Ready(Err(anyhow::anyhow!("Shutting down")));
        }

        fn address(step: Step) -> String {
            trace!("{:?}", step);
            step.0
        }

        match (self.binding.clone(), self.current.take()) {
            (Some(binding), None) => match self.next_step() {
                Some(step) => {
                    trace!("Starting sequence");
                    let _binding = binding.clone();
                    self.current = Some(binding.node.clone().runtime.unwrap().spawn(async move {
                        let mut set = JoinSet::new();

                        let _binding = _binding.clone();
                        for _step in step {
                            let _binding = _binding.clone();
                            set.spawn(async move {
                                trace!("Starting {:?}", _step);
                                if let Some(handle) = _binding.engine_handle().await {
                                    handle.run(address(_step)).await
                                } else {
                                    Err(anyhow::anyhow!("Engine handle is not enabled"))
                                }
                            });
                        }

                        let mut last = Err(anyhow::anyhow!("Not started"));
                        while let Some(result) = set.join_next().await {
                            last = result?;
                        }

                        last
                    }));
                }
                None => {
                    trace!("Done");
                    return Poll::Ready(Err(anyhow::anyhow!("Sequence has completed")));
                }
            },
            (Some(binding), Some(mut current)) => match current.poll_unpin(cx) {
                Poll::Ready(Ok(result)) => match self.next_step() {
                    Some(next) => {
                        trace!("Starting sequence");
                        let _binding = binding.clone();
                        self.current =
                            Some(binding.node.clone().runtime.unwrap().spawn(async move {
                                let mut set = JoinSet::new();

                                let _binding = _binding.clone();
                                for _step in next {
                                    let _binding = _binding.clone();
                                    set.spawn(async move {
                                        trace!("Starting {:?}", _step);
                                        if let Some(handle) = _binding.engine_handle().await {
                                            handle.run(address(_step)).await
                                        } else {
                                            Err(anyhow::anyhow!("Engine handle is not enabled"))
                                        }
                                    });
                                }

                                let mut last = Err(anyhow::anyhow!("Not started"));
                                while let Some(result) = set.join_next().await {
                                    last = result?;
                                }

                                last
                            }));
                    }
                    None => return Poll::Ready(result),
                },
                Poll::Ready(Err(err)) => {
                    error!("{err}");
                    return Poll::Ready(Err(err.into()));
                }
                Poll::Pending => {
                    self.current = Some(current);
                }
            },
            _ => {
                trace!("not bound");
                return Poll::Ready(Err(anyhow::anyhow!(
                    "Sequence has not been bound to a thunk context"
                )));
            }
        }

        cx.waker().wake_by_ref();
        std::task::Poll::Pending
    }
}

/// A step is an operation address to execute on an engine,
///
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Step(pub String, pub StepType);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum StepType {
    /// Indicates that the step should only execute once,
    ///
    Once,
    /// Indicates that the step should execute next,
    ///
    Next,
}

impl FromStr for Step {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Err(anyhow::anyhow!("Step requires an action name"))
        } else {
            Ok(Step(s.to_string(), StepType::Next))
        }
    }
}

impl FromStr for Sequence {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Sequence {
            name: s.to_string(),
            tag: None,
            step: vec![].into(),
            _loop: false,
            _step_list: None,
            binding: None,
            current: None,
            plugin: ResourceKey::default(),
            node: ResourceKey::default(),
        })
    }
}

impl Action for Sequence {
    #[inline]
    fn address(&self) -> String {
        if let Some(tag) = self.tag.as_ref() {
            format!("{}#{}", self.name, tag)
        } else {
            self.name.to_string()
        }
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
    fn node_rk(&self) -> ResourceKey<reality::attributes::Node> {
        self.node
    }

    #[inline]
    fn bind_plugin(&mut self, plugin: ResourceKey<reality::attributes::Attribute>) {
        self.plugin = plugin;
    }

    #[inline]
    fn plugin_rk(&self) -> ResourceKey<reality::attributes::Attribute> {
        self.plugin
    }
}

/// Wrapper over a queue of decorated comma-delimitted steps,
///
#[derive(Clone, Debug)]
struct StepList(VecDeque<Decorated<Delimitted<',', Step>>>);

impl Iterator for StepList {
    type Item = Vec<Step>;

    fn next(&mut self) -> Option<Self::Item> {
        let StepList(queue) = self;

        if let Some(mut front) = queue.pop_front() {
            let prop = front
                .property("kind")
                .map(|k| match k.as_str() {
                    "once" => StepType::Once,
                    _ => StepType::Next,
                })
                .unwrap_or(StepType::Next);

            front.value.as_mut().map(|f| {
                f.map(|mut s| {
                    s.1 = prop;
                    s
                })
                .collect::<Vec<_>>()
            })
        } else {
            None
        }
    }
}

#[tokio::test]
#[tracing_test::traced_test]
async fn test_seq() -> anyhow::Result<()> {
    let mut workspace = Workspace::new();
    workspace.add_buffer(
        "demo.md",
        r#"
    ```runmd
    + .operation a
    <t/demo.testseq>          Hello World a
    
    + .operation b
    <t/demo.testseq>          Hello World b
    
    + .operation c
    <t/demo.testseq>          Hello World c
    
    + .operation d
    <t/demo.testseq>          Hello World d
    
    # -- Test sequence decorations
    + .sequence test
    |# name = Test sequence
    
    # -- Operations on a step execute all at once
    :  .step a, b, c # test-break

    # -- If kind is set to once, this row only executes once if the sequence loops
    : .step b, d,
    |# kind = once

    # -- If this were set to true, then the sequence would automatically loop
    : .loop false

    + .host test-host
    : .action   b/t/demo.testseq
    ```
    "#,
    );

    let mut engine = crate::prelude::DefaultEngine.new();
    engine.enable::<TestSeq>();

    let engine = engine.compile(workspace).await?;

    let eh = engine.engine_handle();
    let _e = engine.spawn(|_, p| {
        eprintln!("{:?}", p);
        Some(p)
    });

    let seq = eh.hosted_resource("engine://test").await.unwrap();
    seq.spawn().await?.unwrap();
    seq.spawn().await?.unwrap();
    seq.spawn().await?.unwrap();
    seq.spawn().await?.unwrap();

    let testseq = eh
        .hosted_resource("test-host://b/t/demo.testseq")
        .await
        .unwrap();
    let testseq = testseq.context().initialized::<TestSeq>().await;
    assert_eq!(testseq.counter, 5);

    Ok(())
}

#[derive(Reality, Default, Clone, Debug)]
#[reality(call = call_test_seq, plugin, group = "demo")]
struct TestSeq {
    #[reality(derive_fromstr)]
    name: String,
    counter: usize,
}

async fn call_test_seq(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let mut init = tc.initialized::<TestSeq>().await;
    init.counter += 1;
    eprintln!(
        "{:?}: {} {}",
        tc.attribute.transmute::<TestSeq>(),
        init.name,
        init.counter
    );
    let key = tc.attribute.transmute::<TestSeq>();
    tc.node().await.lazy_dispatch_mut(move |node| {
        if let Some(mut seq) = node.resource_mut(key) {
            seq.counter = init.counter;
        }
    });
    Ok(())
}
