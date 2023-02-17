use std::sync::Arc;

use toml_edit::Item;

/// Struct for build errors,
///
#[derive(Debug, Default)]
pub struct Error {
    error: Option<Arc<dyn std::error::Error>>,
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self { error: Some(Arc::new(value)) }
    }
}

impl From<std::fmt::Error> for Error {
    fn from(value: std::fmt::Error) -> Self {
        Self { error: Some(Arc::new(value)) }
    }
}

