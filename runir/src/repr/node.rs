use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use crate::define_intern_table;
use crate::push_tag;

use crate::prelude::*;

// Intern table for symbol values
define_intern_table!(SYMBOL: String);

// Intern table for input values
define_intern_table!(INPUT: String);

// Intern table for tag values
define_intern_table!(TAG: String);

// Intern table for path values
define_intern_table!(PATH: String);

// Intern table for node index values
define_intern_table!(NODE_IDX: usize);

// Intern table for source
define_intern_table!(SOURCE: String);

// Intern table for doc headers
define_intern_table!(DOC_HEADERS: Vec<String>);

// Intern table for node level annotations
define_intern_table!(ANNOTATIONS: BTreeMap<String, String>);

/// Node level is a dynamic level of representation,
///
/// Node level asserts and records the input paramters for some resource as well as ordinality.
///
#[derive(Clone)]
pub struct NodeLevel {
    /// Symbol representing this node,
    ///
    symbol: Option<Tag<String, Arc<String>>>,
    /// Runmd expression representing this resource,
    ///
    input: Option<Tag<String, Arc<String>>>,
    /// Tag value assigned to this resource,
    ///
    tag: Option<Tag<String, Arc<String>>>,
    /// Path value assigned to this resource,
    ///
    path: Option<Tag<String, Arc<String>>>,
    /// Node idx,
    ///
    idx: Option<Tag<usize, Arc<usize>>>,
    /// Node source,
    ///
    source: Option<Tag<String, Arc<String>>>,
    /// Node doc headers,
    ///
    doc_headers: Option<Tag<Vec<String>, Arc<Vec<String>>>>,
    /// Node annotations,
    ///
    annotations: Option<Tag<BTreeMap<String, String>, Arc<BTreeMap<String, String>>>>,
}

impl NodeLevel {
    /// Returns a new empty node level,
    ///
    pub fn new() -> Self {
        Self {
            symbol: None,
            input: None,
            tag: None,
            path: None,
            idx: None,
            source: None,
            doc_headers: None,
            annotations: None,
        }
    }

    /// Creates a new input level representation,
    ///
    pub fn new_with(
        symbol: Option<impl Into<String>>,
        input: Option<impl Into<String>>,
        tag: Option<impl Into<String>>,
        path: Option<impl Into<String>>,
        idx: Option<usize>,
        source: Option<impl Into<String>>,
        doc_headers: Option<Vec<impl Into<String>>>,
        annotations: Option<BTreeMap<String, String>>,
    ) -> Self {
        let mut node = Self::new();

        if let Some(symbol) = symbol {
            node = node.with_symbol(symbol);
        }
        if let Some(input) = input {
            node = node.with_input(input);
        }
        if let Some(tag) = tag {
            node = node.with_tag(tag)
        }
        if let Some(path) = path {
            node = node.with_path(path);
        }
        if let Some(idx) = idx {
            node = node.with_idx(idx);
        }
        if let Some(source) = source {
            node = node.with_source(source);
        }
        if let Some(doc_headers) = doc_headers {
            node = node.with_doc_headers(doc_headers);
        }
        if let Some(annotations) = annotations {
            node = node.with_annotations(annotations);
        }

        node
    }

    /// Returns the node level w/ symbol tag set,
    ///
    #[inline]
    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.set_symbol(symbol);
        self
    }

    /// Returns the node level w/ input tag set,
    ///
    #[inline]
    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.set_input(input);
        self
    }

    /// Returns the node level w/ tag tag set,
    ///
    #[inline]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.set_tag(tag);
        self
    }

    /// Returns the node level w/ path tag set,
    ///  
    #[inline]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.set_path(path);
        self
    }

    /// Returns the node level w/ idx tag set,
    ///
    #[inline]
    pub fn with_idx(mut self, idx: usize) -> Self {
        self.set_idx(idx);
        self
    }

    /// Returns the node level w/ source set,
    ///
    #[inline]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.set_source(source);
        self
    }

    /// Returns the node level w/ doc headers set,
    ///
    #[inline]
    pub fn with_doc_headers(mut self, doc_headers: Vec<impl Into<String>>) -> Self {
        self.set_doc_headers(doc_headers);
        self
    }

    /// Returns the node level w/ annotations set,
    ///
    #[inline]
    pub fn with_annotations(mut self, annotations: BTreeMap<String, String>) -> Self {
        self.set_annotations(annotations);
        self
    }

    /// Sets the symbol tag for the node level,
    ///
    #[inline]
    pub fn set_symbol(&mut self, symbol: impl Into<String>) {
        self.symbol = Some(Tag::new(&SYMBOL, Arc::new(symbol.into())));
    }

    /// Returns the node level w/ tag tag set,
    ///
    #[inline]
    pub fn set_input(&mut self, input: impl Into<String>) {
        self.input = Some(Tag::new(&INPUT, Arc::new(input.into())));
    }

    /// Returns the node level w/ tag tag set,
    ///
    #[inline]
    pub fn set_tag(&mut self, tag: impl Into<String>) {
        self.tag = Some(Tag::new(&TAG, Arc::new(tag.into())));
    }

    /// Sets the path tag for the node level,
    ///
    #[inline]
    pub fn set_path(&mut self, path: impl Into<String>) {
        self.path = Some(Tag::new(&PATH, Arc::new(path.into())));
    }

    /// Returns the node level w/ idx tag set,
    ///
    #[inline]
    pub fn set_idx(&mut self, idx: usize) {
        self.idx = Some(Tag::new(&NODE_IDX, Arc::new(idx)));
    }

    /// Sets the source tag for the node level,
    ///
    #[inline]
    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = Some(Tag::new(&SOURCE, Arc::new(source.into())));
    }

    /// Sets the doc headers tag for the node level,
    ///
    #[inline]
    pub fn set_doc_headers(&mut self, mut headers: Vec<impl Into<String>>) {
        self.doc_headers = Some(Tag::new(
            &DOC_HEADERS,
            Arc::new(headers.drain(..).map(|s| s.into()).collect()),
        ))
    }

    /// Returns the node level w/ annotations set,
    ///
    #[inline]
    pub fn set_annotations(&mut self, annotations: BTreeMap<String, String>) {
        self.annotations = Some(Tag::new(&ANNOTATIONS, Arc::new(annotations)));
    }
}

impl Level for NodeLevel {
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
        if let Some(symbol) = self.symbol.as_ref() {
            push_tag!(dyn interner, symbol);
        }

        if let Some(input) = self.input.as_ref() {
            push_tag!(dyn interner, input);
        }

        if let Some(tag) = self.tag.as_ref() {
            push_tag!(dyn interner, tag);
        }

        if let Some(path) = self.path.as_ref() {
            push_tag!(dyn interner, path);
        }

        if let Some(idx) = self.idx.as_ref() {
            push_tag!(dyn interner, idx);
        }

        if let Some(docs) = self.doc_headers.as_ref() {
            push_tag!(dyn interner, docs);
        }

        if let Some(source) = self.source.as_ref() {
            push_tag!(dyn interner, source);
        }

        if let Some(annotations) = self.annotations.as_ref() {
            push_tag!(dyn interner, annotations);
        }

        interner.set_level_flags(LevelFlags::LEVEL_2);

        interner.interner()
    }

    type Mount = (
        // Symbol
        Option<Arc<String>>,
        // Input
        Option<Arc<String>>,
        // Tag
        Option<Arc<String>>,
        // Path
        Option<Arc<String>>,
        // Doc headers
        Option<Arc<Vec<String>>>,
        // Annotations
        Option<Arc<BTreeMap<String, String>>>,
    );

    #[inline]
    fn mount(&self) -> Self::Mount {
        (
            self.symbol.as_ref().map(|i| i.create_value.clone()),
            self.input.as_ref().map(|i| i.create_value.clone()),
            self.tag.as_ref().map(|t| t.create_value.clone()),
            self.path.as_ref().map(|p| p.create_value.clone()),
            self.doc_headers.as_ref().map(|t| t.create_value.clone()),
            self.annotations.as_ref().map(|a| a.create_value.clone()),
        )
    }
}

/// Wrapper struct with access to node tags,
///
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub struct NodeRepr(pub(crate) InternHandle);

impl NodeRepr {
    /// Returns the node symbol,
    ///
    pub async fn symbol(&self) -> Option<Arc<String>> {
        self.0.symbol().await
    }

    /// Returns node input,
    ///
    #[inline]
    pub async fn input(&self) -> Option<Arc<String>> {
        self.0.input().await
    }

    /// Returns node path,
    ///
    #[inline]
    pub async fn path(&self) -> Option<Arc<String>> {
        self.0.path().await
    }

    /// Returns node tag,
    ///
    #[inline]
    pub async fn tag(&self) -> Option<Arc<String>> {
        self.0.tag().await
    }

    /// Returns the node idx,
    ///
    #[inline]
    pub async fn idx(&self) -> Option<usize> {
        self.0.node_idx().await
    }

    /// Returns the node source,
    ///
    #[inline]
    pub async fn source(&self) -> Option<Arc<String>> {
        self.0.node_source().await
    }

    /// Returns node doc_headers,
    ///
    #[inline]
    pub async fn doc_headers(&self) -> Option<Arc<Vec<String>>> {
        self.0.doc_headers().await
    }

    /// Returns node annotations,
    ///
    #[inline]
    pub async fn annotations(&self) -> Option<Arc<BTreeMap<String, String>>> {
        self.0.annotations().await
    }

    /// Tries to return the symbol,
    ///
    #[inline]
    pub fn try_symbol(&self) -> Option<Arc<String>> {
        self.0.try_symbol()
    }

    /// Tries to returns node input,
    ///
    #[inline]
    pub fn try_input(&self) -> Option<Arc<String>> {
        self.0.try_input()
    }

    /// Tries to return node tag,
    ///
    #[inline]
    pub fn try_tag(&self) -> Option<Arc<String>> {
        self.0.try_tag()
    }

    /// Tries to return node path,
    ///
    #[inline]
    pub fn try_path(&self) -> Option<Arc<String>> {
        self.0.try_path()
    }

    /// Tries to return the node source,
    ///
    #[inline]
    pub fn try_source(&self) -> Option<Arc<String>> {
        self.0.try_node_source()
    }

    /// Tries to return node doc_headers,
    ///
    #[inline]
    pub fn try_doc_headers(&self) -> Option<Arc<Vec<String>>> {
        self.0.try_doc_headers()
    }

    /// Tries to return node annotations,
    ///
    #[inline]
    pub fn try_annotations(&self) -> Option<Arc<BTreeMap<String, String>>> {
        self.0.try_annotations()
    }
}
