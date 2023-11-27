use std::collections::BTreeMap;
use std::collections::HashMap;

use reality::Attribute;
use reality::HostedResource;
use reality::ParsedBlock;
use reality::ResourceKey;
use serde::Deserialize;
use serde::Serialize;

use crate::prelude::Action;

/// Deck is an index of resource keys to parsed metadata derived
/// from the project of an engine compilation.
///
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    /// Doc headers,
    ///
    doc_headers: HashMap<ResourceKey<Attribute>, Vec<String>>,
    /// Properties,
    ///
    properties: HashMap<ResourceKey<Attribute>, BTreeMap<String, String>>,
    /// Paths,
    ///
    pub paths: BTreeMap<String, ResourceKey<Attribute>>,
    /// Node paths,
    ///
    node_paths: BTreeMap<NodePath, ResourceKey<Attribute>>,
}

impl Deck {
    /// Returns doc headers,
    /// 
    pub fn doc_headers(&self, hr: &HostedResource)  -> Option<&Vec<String>> {
        let key = Self::key(hr);
        eprintln!("Deck key -- {:?}", key);
        self.doc_headers.get(&key)
    }

    /// Returns a deck key from a hosted resource,
    /// 
    fn key(hr: &HostedResource) -> ResourceKey<Attribute> {
        hr.node_rk().branch(hr.plugin_rk()).transmute()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodePath {
    pub node: usize,
    pub offset: usize,
}

impl From<&ParsedBlock> for Deck {
    fn from(value: &ParsedBlock) -> Self {
        let mut deck = Deck::default();

        for (idx, (nk, node)) in value.nodes.iter().enumerate() {
            for (offset, (rk, props)) in node.comment_properties.iter().enumerate() {
                // deck.properties.insert(*rk, props.clone());
                let rk = nk.branch(rk);
                deck.properties.insert(rk.transmute(), props.clone());
                deck.node_paths.insert(
                    NodePath {
                        node: idx,
                        offset,
                    },
                    rk.transmute(),
                );
            }

            for (offset, (rk, docs)) in node.doc_headers.iter().enumerate() {
                // deck.doc_headers.insert(*rk, docs.clone());
                let rk = nk.branch(rk);
                deck.doc_headers.insert(rk.transmute(), docs.clone());
                deck.node_paths.insert(
                    NodePath {
                        node: idx,
                        offset,
                    },
                    rk.transmute(),
                );
            }

            for (offset, (rk, props)) in node.properties.comment_properties.iter().enumerate() {
                // deck.properties.insert(rk.transmute(), props.clone());
                let rk = nk.branch(rk);
                deck.properties.insert(rk.transmute(), props.clone());
                deck.node_paths.insert(
                    NodePath {
                        node: idx,
                        offset,
                    },
                    rk.transmute(),
                );
            }

            for (offset, (rk, docs)) in node.properties.doc_headers.iter().enumerate() {
                // deck.doc_headers.insert(rk.transmute(), docs.clone());
                let rk = nk.branch(rk);
                deck.doc_headers.insert(rk.transmute(), docs.clone());
                deck.node_paths.insert(
                    NodePath {
                        node: idx,
                        offset,
                    },
                    rk.transmute(),
                );
            }

            for (idx, attr) in node.attributes.iter().enumerate() {
                if let Some(defined) = node.properties.defined.get(attr) {
                    for (offset, defined) in defined.iter().enumerate() {
                        deck.node_paths.insert(
                            NodePath {
                                node: idx,
                                offset,
                            },
                            defined.transmute(),
                        );
                    }
                }
            }

            for (path, rk) in node.paths.iter().filter(|(f, _)| !f.is_empty()) {
                deck.paths.insert(path.to_string(), nk.branch(rk).transmute());
            }
        }

        for (path, rk) in value.paths.iter().filter(|(f, _)| !f.is_empty()) {
            deck.paths.insert(path.to_string(), rk.transmute());
        }
        for (path, rk) in value.resource_paths.iter().filter(|(f, _)| !f.is_empty()) {
            deck.paths.insert(path.to_string(), rk.rk);
        }

        deck
    }
}
