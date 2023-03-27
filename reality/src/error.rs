use std::fmt::Display;
use std::sync::Arc;

use tokio::task::JoinError;

/// Struct for build errors,
///
#[derive(Clone, Debug, Default)]
pub struct Error {
    error: Option<Arc<dyn std::error::Error + Send + Sync + 'static>>,
    message: Option<String>,
}

impl std::error::Error for Error {}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self {
            error: None,
            message: Some(format!("{msg}")),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(result) = self.error.as_ref().map(|e| write!(f, "{} ", e)) {
            result?;
        }

        if let Some(result) = self.message.as_ref().map(|msg| write!(f, "{}", msg)) {
            result?;
        }

        Ok(())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self {
            error: Some(Arc::new(value)),
            message: None,
        }
    }
}

impl From<std::fmt::Error> for Error {
    fn from(value: std::fmt::Error) -> Self {
        Self {
            error: Some(Arc::new(value)),
            message: None,
        }
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self {
            error: None,
            message: Some(value),
        }
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self {
            error: None,
            message: Some(value.to_string()),
        }
    }
}

impl From<specs::error::Error> for Error {
    fn from(value: specs::error::Error) -> Self {
        Self {
            error: Some(Arc::new(value)),
            message: None
        }
    }
}

impl From<JoinError> for Error {
    fn from(value: JoinError) -> Self {
        Self { error: Some(Arc::new(value)), message: None }
    }
}