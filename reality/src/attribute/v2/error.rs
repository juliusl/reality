use std::sync::Arc;

use toml_edit::Item;

/// Struct for build errors,
///
#[derive(Debug, Default)]
pub struct Error {
    error: Option<Arc<dyn std::error::Error>>,
    message: Option<String>,
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self { error: Some(Arc::new(value)), message: None }
    }
}

impl From<std::fmt::Error> for Error {
    fn from(value: std::fmt::Error) -> Self {
        Self { error: Some(Arc::new(value)), message: None }
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self { error: None, message: Some(value) }
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self { error: None, message: Some(value.to_string()) }
    }
}

