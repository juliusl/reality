use std::fmt::Display;

use reality::prelude::*;
use serde::Deserialize;
use serde::Serialize;

/// Struct containing address parameters and a notification handle,
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
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
    /// Filter value of this address,
    ///
    filter: Option<String>,
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
            filter: None,
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

    /// Sets the filter paramter of this address,
    ///
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Returns the value of the node identifier of this address,
    /// 
    pub fn node(&self) -> &str {
        self.node.as_str()
    }

    /// Returns the node address,
    ///
    pub fn node_address(&self) -> String {
        if let Some(host) = self.host.as_ref() {
            format!("{}://{}", host, self.node)
        } else {
            format!("engine://{}", self.node)
        }
    }

    /// Returns the path,
    ///
    /// **Note**: Will trim the leading `/`
    ///
    pub fn path(&self) -> &str {
        self.path.trim_start_matches('/')
    }

    pub fn filter_str(&self) -> Option<&str> {
        self.filter.as_ref().map(|f| f.as_str())
    }

    /// Returns the filter as a form_urlencoded Parser,
    ///
    pub fn filter(&self) -> Option<url::form_urlencoded::Parse<'_>> {
        self.filter
            .as_ref()
            .map(|f| url::form_urlencoded::parse(f.as_bytes()))
    }

    /// TODO: Use the resource key to build paths to fields?
    ///
    fn _apply_filter(&self) -> anyhow::Result<()> {
        if let Some(filter) = self.filter.as_ref() {
            let filter = url::form_urlencoded::parse(filter.as_bytes());
            for (k, v) in filter {
                eprintln!("{k}: {v}");
            }
        }
        Ok(())
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
            filter: Option<&str>,
        ) -> anyhow::Result<Address> {
            match (node, tag, filter) {
                (None, None, None) if path == "" => { // Node address
                    Ok(Address::new(String::new()).with_host(host))
                },
                (None, None, Some(filter)) if path == "" => { // Node address
                    Ok(Address::new(String::new()).with_host(host).with_filter(filter))
                },
                (None, Some(tag), Some(filter)) if path == "" => { // Node address
                    Ok(Address::new(String::new()).with_host(host).with_tag(tag).with_filter(filter))
                },
                (None, _, _) => {
                    Err(anyhow::anyhow!("Address cannot be constructed without a bound engine action -- {host} :// {path} {:?} {:?} {:?}", node, tag, filter))
                },
                (Some(node), None, None) => {
                    Ok(Address::new(node.to_string()).with_host(host).with_path(path))
                },
                (Some(node), Some(tag), None) => {
                    Ok(Address::new(node.to_string()).with_host(host).with_path(path).with_tag(tag))
                },
                (Some(node), None, Some(filter)) => {
                    Ok(Address::new(node.to_string()).with_host(host).with_path(path).with_filter(filter))
                },
                (Some(node), Some(tag), Some(filter)) => {
                    Ok(Address::new(node.to_string()).with_host(host).with_path(path).with_tag(tag).with_filter(filter))
                },
            }
        }

        match url::Url::parse(s) {
            Ok(url) => Ok(configure_node_tag(
                url.scheme(),
                url.path(),
                url.host_str(),
                url.fragment(),
                url.query(),
            )?),
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                match url::Url::parse(format!("engine://{s}").as_str()) {
                    Ok(url) => Ok(configure_node_tag(
                        url.scheme(),
                        url.path(),
                        url.host_str(),
                        url.fragment(),
                        url.query(),
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

    let address = Address::from_str(
        "test://show_demo_window/a/loopio.test?type_name=core::alloc::String#test",
    )
    .expect("should be valid");
    assert_eq!(address.host, Some("test".to_string()));
    assert_eq!(address.tag, Some("test".to_string()));
    assert_eq!(address.node, "show_demo_window".to_string());
    assert_eq!(address.path, "/a/loopio.test".to_string());
    assert_eq!(
        address.filter,
        Some("type_name=core::alloc::String".to_string())
    );
    address._apply_filter().unwrap();

    let address = Address::from_str("engine://?event=test-event").unwrap();
    eprintln!("{}", address);

    for (k, v) in address.filter().unwrap() {
        eprintln!("{k}: {v}")
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(host) = self.host.as_ref() {
            write!(f, "{}://", host)?;
        }

        write!(f, "{}", self.node)?;

        write!(f, "{}", self.path)?;

        if let Some(filter) = self.filter.as_ref() {
            write!(f, "?{}", filter)?;
        }

        if let Some(tag) = self.tag.as_ref() {
            write!(f, "#{}", tag)?;
        }

        Ok(())
    }
}
