use std::fmt::Display;

use reality::prelude::*;
use serde::{Deserialize, Serialize};

/// Common trait for engine node types,
///
pub trait Action {
    /// Return the address of an action,
    ///
    fn address(&self) -> String;

    /// Bind a thunk context to the action,
    ///
    /// **Note** This context has access to the compiled node this action corresponds to.
    ///
    fn bind(&mut self, context: ThunkContext);

    /// Returns the current context,
    ///
    /// **Note** Should panic if currently unbound,
    ///
    fn context(&self) -> &ThunkContext;

    /// Returns a mutable reference to the current context,
    ///
    /// **Note** Should panic if currently unbound,
    ///
    fn context_mut(&mut self) -> &mut ThunkContext;
}

/// Struct containing address parameters and a notification handle,
///
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Address {
    /// Host setting of this address,
    ///
    host: Option<String>,
    /// Address to the node, (ex. operation, sequence, etc),
    ///
    node: String,
    /// Path assignment value, (ex. <a/ext-name>),
    ///
    path: String,
    /// Tag value of this address,
    ///
    tag: Option<String>,
}

impl Address {
    /// Returns a new address on a node,
    ///
    pub fn new(node: String) -> Self {
        Self {
            host: None,
            node,
            tag: None,
            path: String::new(),
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
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Sets the tag parameter of the address,
    ///
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
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
    pub fn matches(
        &self,
        node: impl AsRef<str>,
        path: impl AsRef<str>,
        host: Option<&str>,
        tag: Option<&str>,
    ) -> bool {
        self.node == node.as_ref()
            && self.path == path.as_ref()
            && self.host.as_ref().map(String::as_str) == host
            && self.tag.as_ref().map(String::as_str) == tag
    }

    /// Returns the node address,
    ///
    pub fn node_address(&self) -> String {
        if let Some(host) = self.host.as_ref() {
            format!(
                "{}://{}",
                host,
                self.node
            )
        } else {
            format!(
                "engine://{}",
                self.node
            )
        }
    }

    /// Returns the path,
    /// 
    /// **Note**: Will trim the leading `/`
    /// 
    pub fn path(&self) -> &str {
        self.path.trim_start_matches('/')
    }
}

impl FromStr for Address {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        /// Inner function, configures a new Address,
        ///
        fn configure_node_tag(
            host: &str,
            path: &str,
            node: Option<&str>,
            tag: Option<&str>,
        ) -> anyhow::Result<Address> {
            match (node, tag) {
                (None, _) if path == "" => {
                    Ok(Address::new(String::new()).with_host(host))
                },
                (None, _) => {
                    Err(anyhow::anyhow!("Address cannot be constructed without a bound engine action -- {host} :// {path} {:?} {:?}", node, tag))
                },
                (Some(node), None) => {
                    Ok(Address::new(node.to_string()).with_host(host).with_path(path))
                },
                (Some(node), Some(tag)) => {
                    Ok(Address::new(node.to_string()).with_host(host).with_path(path).with_tag(tag))
                },
            }
        }

        match url::Url::parse(s) {
            Ok(url) => Ok(configure_node_tag(
                url.scheme(),
                url.path(),
                url.host_str(),
                url.fragment(),
            )?),
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                match url::Url::parse(format!("engine://{s}").as_str()) {
                    Ok(url) => Ok(configure_node_tag(
                        url.scheme(),
                        url.path(),
                        url.host_str(),
                        url.fragment(),
                    )?),
                    Err(err) => Err(anyhow::anyhow!("Could not parse address: {err}")),
                }
            }
            Err(err) => Err(anyhow::anyhow!("Could not parse address {err}")),
        }
    }
}

#[test]
fn test_address_from_str() {
    let address = Address::from_str("show_demo_window/a/loopio.test").expect("should be valid");
    assert_eq!(address.host, Some("engine".to_string()));
    assert_eq!(address.node, "show_demo_window".to_string());
    assert_eq!(address.path, "/a/loopio.test".to_string());

    let _ = Address::from_str("/loopio.test").expect_err("Cannot specify a path w/o a node");

    let address = Address::from_str("test://").expect("should be valid");
    assert_eq!(address.host, Some("test".to_string()));

    let address =
        Address::from_str("test://show_demo_window/a/loopio.test#test").expect("should be valid");
    assert_eq!(address.host, Some("test".to_string()));
    assert_eq!(address.tag, Some("test".to_string()));
    assert_eq!(address.node, "show_demo_window".to_string());
    assert_eq!(address.path, "/a/loopio.test".to_string());
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(host) = self.host.as_ref() {
            write!(f, "{}://", host)?;
        }

        write!(f, "{}", self.node)?;

        write!(f, "{}", self.path)?;

        if let Some(tag) = self.tag.as_ref() {
            write!(f, "#{}", tag)?;
        }

        Ok(())
    }
}

impl Action for HostedResource {
    fn address(&self) -> String {
        self.address.to_string()
    }

    fn bind(&mut self, context: ThunkContext) {
        self.binding = Some(context);
    }

    fn context(&self) -> &ThunkContext {
        self.binding.as_ref().expect("should be bound to an engine")
    }

    fn context_mut(&mut self) -> &mut ThunkContext {
        self.binding.as_mut().expect("should be bound to an engine")
    }
}