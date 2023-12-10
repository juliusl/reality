use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

/// Struct for storing extra information on an runmd instruction,
///
#[derive(Hash, Clone, Debug, Serialize, PartialEq, Eq, Deserialize, Default)]
pub struct Decoration {
    /// Doc headers pushed on the current instruction,
    ///
    pub doc_headers: Option<Vec<String>>,
    /// Properties parsed from the comments on this instruction,
    ///
    pub comment_properties: Option<BTreeMap<String, String>>,
}

impl Decoration {
    /// Returns a slice of doc lines,
    ///
    pub fn docs(&self) -> Option<&[String]> {
        self.doc_headers.as_deref()
    }

    /// Returns the value of a comment property added to this instruction,
    ///
    pub fn prop(&self, key: impl AsRef<str>) -> Option<&String> {
        self.comment_properties
            .as_ref()
            .and_then(|p| p.get(key.as_ref()))
    }

    /// Returns a pointer to all properties,
    ///
    pub fn props(&self) -> &BTreeMap<String, String> {
        static EMPTY: BTreeMap<String, String> = BTreeMap::new();

        self.comment_properties.as_ref().unwrap_or(&EMPTY)
    }
}
