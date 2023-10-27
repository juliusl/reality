use std::collections::VecDeque;

use reality::prelude::*;

use crate::engine::EngineHandle;

/// Struct containing steps of a sequence of operations,
///
#[derive(Reality, Clone, Debug)]
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
    /// Handle to engine to execute sequence w/
    ///
    #[reality(ignore)]
    _engine: Option<EngineHandle>,
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
            _engine: None,
        }
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
    pub fn bind(&self, engine: EngineHandle) -> Self {
        let mut clone = self.clone();
        clone._engine = Some(engine);
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
            _engine: None,
        })
    }
}
