use std::fmt::Display;
use std::sync::Arc;

use tokio::task::JoinError;

/// Struct for build errors,
///
#[derive(Clone, Debug, Default)]
pub struct Error {
    error: Option<Arc<dyn std::error::Error + Send + Sync + 'static>>,
    message: Option<String>,
    static_error: Option<StaticError>,
}

/// Struct for static errors,
/// 
#[derive(Clone, Debug, Default)]
struct StaticError {
    message: &'static str,
}

impl StaticError {
    pub const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl std::error::Error for StaticError {}

impl Display for StaticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error {
    /// Returns a new static error,
    ///
    pub const fn new(err: &'static str) -> Self {
        Error {
            error: None,
            message: None,
            static_error: Some(StaticError::new(err)),
        }
    }

    /// Returns a not_implemented error,
    /// 
    pub const fn not_implemented() -> Self {
        const NOT_IMPLEMENTED: Error = Error::new("Not implemented");
        NOT_IMPLEMENTED
    }

    /// Returns a skip error,
    /// 
    pub const fn skip() -> Self {
        const SKIP: Error = Error::new("Skip");
        SKIP
    }
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
            static_error: None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(err) = self.static_error.as_ref() {
            return write!(f, "{}", err);
        }

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
            static_error: None,
        }
    }
}

impl From<std::fmt::Error> for Error {
    fn from(value: std::fmt::Error) -> Self {
        Self {
            error: Some(Arc::new(value)),
            message: None,
            static_error: None,
        }
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self {
            error: None,
            message: Some(value),
            static_error: None,
        }
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self {
            error: None,
            message: Some(value.to_string()),
            static_error: None,
        }
    }
}

impl From<specs::error::Error> for Error {
    fn from(value: specs::error::Error) -> Self {
        Self {
            error: Some(Arc::new(value)),
            message: None,
            static_error: None,
        }
    }
}

impl From<JoinError> for Error {
    fn from(value: JoinError) -> Self {
        Self {
            error: Some(Arc::new(value)),
            message: None,
            static_error: None,
        }
    }
}
