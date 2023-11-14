use std::sync::Arc;

use tokio::sync::Notify;

/// Struct containing address parameters and a notification handle,
///
pub struct Address {
    /// Parent of this address,
    ///
    parent: String,
    /// Path of this address,
    ///
    path: String,
    /// Notified whenever the address is accessed,
    ///
    notify: Arc<Notify>,
}

impl Address {
    /// Returns a new address,
    /// 
    pub fn new() -> Self {
        Self {
            parent: String::new(),
            path: String::new(),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Sets the path parameter of the address,
    /// 
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Sets the parent parameter of the address,
    /// 
    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = parent.into();
        self
    }

    /// Returns true if the parent and path match,
    ///
    /// If parent is None, then only the path is checked. If path is Some, then both the parent and path
    /// must match.
    ///
    /// "Matching" means that the assigned path ends w/ the search parameter.
    ///
    /// For example, an address of "loopio.println", would match a path search parameter of "println".
    ///
    pub fn matches(&self, parent: Option<&str>, path: impl AsRef<str>) -> bool {
        if let Some(parent) = parent {
            if self.parent.ends_with(parent) && self.path.ends_with(path.as_ref()) {
                self.notify.notify_waiters();
                true
            } else {
                false
            }
        } else {
            if self.path.ends_with(path.as_ref()) {
                self.notify.notify_waiters();
                true
            } else {
                false
            }
        }
    }
}
