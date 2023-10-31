use std::collections::VecDeque;
use std::task::Poll;

use futures_util::FutureExt;
use reality::prelude::*;
use tokio::task::JoinHandle;
use tracing::{error, trace};

use crate::ext::Ext;

/// Struct containing steps of a sequence of operations,
///
#[derive(Reality)]
pub struct Sequence {
    /// Name of this sequence,
    ///
    #[reality(ignore)]
    pub name: String,
    /// Tag of this sequence,
    ///
    #[reality(ignore)]
    pub tag: Option<String>,
    /// Steps that should be executed only once at the beginning,
    ///
    #[reality(vecdeq_of=Tagged<Step>)]
    once: VecDeque<Tagged<Step>>,
    /// Steps that should be executed one-after the other,
    ///
    #[reality(vecdeq_of=Tagged<Step>)]
    next: VecDeque<Tagged<Step>>,
    /// Indicates the sequence should loop,
    ///
    #[reality(rename = "loop")]
    _loop: bool,
    /// If loop is enabled, instead of popping from next, a cursor will be maintained,
    ///
    _cursor: usize,
    /// ThunkContext this sequence is bounded to,
    ///
    #[reality(ignore)]
    pub binding: Option<ThunkContext>,
    /// Current sequence being run,
    ///
    #[reality(ignore)]
    current: Option<JoinHandle<anyhow::Result<ThunkContext>>>,
}

impl Clone for Sequence {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            tag: self.tag.clone(),
            once: self.once.clone(),
            next: self.next.clone(),
            _loop: self._loop.clone(),
            _cursor: self._cursor.clone(),
            binding: self.binding.clone(),
            current: None,
        }
    }
}

impl std::fmt::Debug for Sequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sequence")
            .field("name", &self.name)
            .field("tag", &self.tag)
            .field("once", &self.once)
            .field("next", &self.next)
            .field("_loop", &self._loop)
            .field("_cursor", &self._cursor)
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
            once: vec![].into(),
            next: vec![].into(),
            _loop: false,
            _cursor: 0,
            binding: None,
            current: None,
        }
    }

    /// Returns true if the sequence is currently active,
    ///
    pub fn is_active(&self) -> bool {
        self.current.is_none()
    }

    /// Returns the address to use w/ this operation,
    ///
    pub fn address(&self) -> String {
        if let Some(tag) = self.tag.as_ref() {
            format!("{}#{}", self.name, tag)
        } else {
            self.name.to_string()
        }
    }

    /// Binds an engine handle to this sequence,
    ///
    pub fn bind(&self, context: ThunkContext) -> Self {
        let mut clone = self.clone();
        clone.binding = Some(context);
        clone
    }

    /// Returns the next operation to run,
    ///
    /// If None is returned, it signals the end of the sequence.
    ///
    /// If _loop is true, after None is returned it will reset the cursor, such
    /// that next() will then return the beginning of the next sequence.
    ///
    pub fn next(&mut self) -> Option<Tagged<Step>> {
        if let Some(tc) = self.binding.as_mut() {
            let mut storage = tc.source.storage.try_write().unwrap();
            storage.drain_dispatch_queues();
        }

        let once = self.once.pop_front();
        if once.is_some() {
            return once;
        }

        if self._loop {
            let next = self.next.get(self._cursor).cloned();
            if next.is_some() {
                self._cursor += 1;
                return next;
            }

            self._cursor = 0;
            None
        } else {
            self.next.pop_front()
        }
    }
}

impl std::future::Future for Sequence {
    type Output = anyhow::Result<ThunkContext>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        fn address(step: Tagged<Step>) -> String {
            match (step.value(), step.tag()) {
                (Some(address), None) => address.0.to_string(),
                (Some(address), Some(_)) => address.0.to_string(),
                _ => String::new(),
            }
        }

        match (self.binding.clone(), self.current.take()) {
            (Some(binding), None) => match self.next() {
                Some(step) => {
                    trace!("Starting sequence");
                    let handle = binding.engine_handle();
                    self.current = Some(
                        binding
                            .source
                            .runtime
                            .unwrap()
                            .spawn(async move { handle.unwrap().run(address(step)).await }),
                    );
                }
                None => {
                    trace!("Done");
                    return Poll::Ready(Err(anyhow::anyhow!("Sequence has completed")));
                }
            },
            (Some(binding), Some(mut current)) => match current.poll_unpin(cx) {
                Poll::Ready(Ok(result)) => {
                    match self.next() {
                        Some(next) => {
                            trace!("Executing next step");
                            let handle = binding.engine_handle();
                            self.current =
                                Some(binding.source.runtime.unwrap().spawn(async move {
                                    handle.unwrap().run(address(next)).await
                                }));
                        }
                        None => return Poll::Ready(result),
                    }
                }
                Poll::Ready(Err(err)) => {
                    error!("{err}");
                    return Poll::Ready(Err(err.into()));
                }
                Poll::Pending => {
                    trace!("continuing to poll");
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
#[derive(Clone, Debug)]
pub struct Step(pub String);

impl FromStr for Step {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Step(s.to_string()))
    }
}

impl FromStr for Sequence {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Sequence {
            name: s.to_string(),
            tag: None,
            once: vec![].into(),
            next: vec![].into(),
            _loop: false,
            _cursor: 0,
            binding: None,
            current: None,
        })
    }
}