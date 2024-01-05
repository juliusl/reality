use std::collections::BTreeMap;
use std::path::PathBuf;

use async_trait::async_trait;
use hyper::client::HttpConnector;
use hyper::Body;
use hyper::Method;
use hyper::Response;
use hyper::Uri;
use poem::http::uri::PathAndQuery;
use poem::http::uri::Scheme;
use reality::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;

use crate::prelude::Action;
use crate::prelude::EngineProxy;

use super::Ext;

/// Type-alias for a secure client,
///
pub type SecureClient = hyper::Client<hyper_tls::HttpsConnector<HttpConnector>>;

pub fn secure_client() -> SecureClient {
    hyper::Client::builder().build(hyper_tls::HttpsConnector::new())
}

/// Type-alias for a local client,
///
pub type LocalClient = hyper::Client<HttpConnector>;

pub fn local_client() -> LocalClient {
    hyper::Client::new()
}

/// Extensions for working w/ a hyper client,
///
#[async_trait]
pub trait HyperExt {
    /// Makes an https request and returns the response,
    ///
    async fn request(
        &mut self,
        request: hyper::Request<hyper::Body>,
        use_https: bool,
    ) -> anyhow::Result<hyper::Response<hyper::Body>>;

    /// Registers an internal host alias,
    ///
    /// When the scheme/host of the alias uri is received, the scheme/host of the replacement will be applied instead.
    ///
    async fn register_internal_host_alias(&mut self, alias: Uri, replace: Uri);

    /// Lookup an alias for a host internall w/ the scheme/host of a uri being resolved,
    ///
    /// If successful, will return a a uri w/ the scheme and host replaced.
    ///
    async fn internal_host_lookup(&mut self, resolve: &Uri) -> Option<Uri>;

    /// Take response if any from storage target,
    ///
    async fn take_response(&mut self) -> Option<hyper::Response<hyper::Body>>;
}

/// DRY - make request
///
macro_rules! do_request {
    ($source:ident, $request:ident, $client:ty) => {
        if let Some(client) = $source.resource::<$client>(ResourceKey::root()) {
            let response = client.request($request).await?;
            Ok(response)
        } else {
            Err(anyhow::anyhow!("Secure http client is not enabled"))
        }
    };
}

#[async_trait]
impl HyperExt for ThunkContext {
    async fn request(
        &mut self,
        request: hyper::Request<hyper::Body>,
        use_https: bool,
    ) -> anyhow::Result<hyper::Response<hyper::Body>> {
        let source = self.node().await;

        if use_https {
            do_request!(source, request, SecureClient)
        } else {
            do_request!(source, request, LocalClient)
        }
    }

    async fn take_response(&mut self) -> Option<hyper::Response<hyper::Body>> {
        self.transient_mut()
            .await
            .take_resource::<Response<Body>>(ResourceKey::root())
            .map(|r| *r)
    }

    /// Registers an internal host alias,
    ///
    /// When the scheme/host of the alias uri is received, the scheme/host of the replacement will be applied instead.
    ///
    async fn register_internal_host_alias(&mut self, alias: Uri, replace: Uri) {
        let _key = (alias.scheme(), alias.host());

        let _value = (
            replace.scheme().cloned(),
            replace.host().map(|s| s.to_string()),
            replace.port_u16(),
        );

        let _init = self.initialized::<EngineProxy>().await;

        // if let Some(eh) = self.engine_handle().await {
        //     if let Ok(host) = eh
        //         .hosted_resource(format!("{}://", alias.scheme_str().unwrap_or_default()))
        //         .await
        //     {
        //         unsafe {
        //             let host = host.context().node_mut().await;
        //             host.put_resource(init, None);

        //             let key =
        //                 ResourceKey::<(Option<Scheme>, Option<String>, Option<u16>)>::with_hash(
        //                     key,
        //                 );
        //             host.put_resource(value, Some(key));
        //         }
        //     }
        // }
    }

    /// Lookup an alias for a host internall w/ the scheme/host of a uri being resolved,
    ///
    /// If successful, will return a a uri w/ the scheme and host replaced.
    ///
    async fn internal_host_lookup(&mut self, resolve: &Uri) -> Option<Uri> {
        let key = (resolve.scheme(), resolve.host());

        if let Some(eh) = self.engine_handle().await {
            let host = if let Ok(host) = eh
                .hosted_resource(format!("{}://", resolve.scheme_str().unwrap_or_default()))
                .await
            {
                let host = host.context().node().await;

                let key =
                    ResourceKey::<(Option<Scheme>, Option<String>, Option<u16>)>::with_hash(key);
                let alias = host
                    .resource::<(Option<Scheme>, Option<String>, Option<u16>)>(key)
                    .as_deref()
                    .cloned();

                match alias {
                    Some(parts) => match parts {
                        (Some(scheme), Some(host), Some(port)) => Uri::builder()
                            .scheme(scheme.clone())
                            .authority(format!("{}:{}", host, port))
                            .path_and_query(
                                resolve
                                    .path_and_query()
                                    .cloned()
                                    .unwrap_or(PathAndQuery::from_static("/")),
                            )
                            .build()
                            .ok(),
                        (Some(scheme), Some(host), None) => Uri::builder()
                            .scheme(scheme.clone())
                            .authority(host.as_str())
                            .path_and_query(
                                resolve
                                    .path_and_query()
                                    .cloned()
                                    .unwrap_or(PathAndQuery::from_static("/")),
                            )
                            .build()
                            .ok(),
                        _ => None,
                    },
                    None => {
                        warn!("Did not find internal host for {:?}", resolve);
                        None
                    }
                }
            } else {
                None
            };

            return host;
        }

        None
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct UriParam(hyper_serde::Serde<Uri>);

impl PartialEq for UriParam {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl std::ops::Deref for UriParam {
    type Target = Uri;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl std::ops::DerefMut for UriParam {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl AsRef<Uri> for UriParam {
    fn as_ref(&self) -> &Uri {
        &self.0 .0
    }
}

impl AsMut<Uri> for UriParam {
    fn as_mut(&mut self) -> &mut Uri {
        &mut self.0 .0
    }
}

impl FromStr for UriParam {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uri: Uri = s.parse()?;

        Ok(UriParam(hyper_serde::Serde(uri)))
    }
}

impl Default for UriParam {
    fn default() -> Self {
        UriParam(hyper_serde::Serde(Uri::from_static("/dev/null")))
    }
}

#[derive(Reality, Deserialize, Serialize, Default, PartialEq, Debug, Clone)]
#[reality(plugin, group = "builtin")]
pub struct Request {
    /// Uri to make request to,
    ///
    #[reality(derive_fromstr)]
    uri: UriParam,
    /// Headers to attach to the request,
    ///
    #[reality(map_of=String)]
    headers: BTreeMap<String, String>,
    /// Http method to use for the request,
    ///
    method: String,
    /// File data to attach to the request,
    ///
    #[reality(option_of=PathBuf)]
    data: Option<PathBuf>,
}

#[async_trait]
impl CallAsync for Request {
    /// Executed by `ThunkContext::spawn`,
    ///
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        let initialized = context.initialized::<Request>().await;

        // println!("Starting request {:?}", initialized);

        let uri = context
            .internal_host_lookup(initialized.uri.as_ref())
            .await
            .unwrap_or(initialized.uri.as_ref().clone());

        // println!("Resolved uri {:?}", uri);
        let mut request = hyper::Request::builder().uri(&uri);

        // Handle setting method on request
        if let Ok(method) = Method::from_str(&initialized.method) {
            request = request.method(method);
        }

        // Handle adding headers
        for (header, value) in initialized
            .headers
            .iter()
            .try_fold(
                BTreeMap::<String, String>::new(),
                |mut acc, (name, value)| {
                    if let Some(previous) = acc.get_mut(name) {
                        use std::fmt::Write;
                        write!(previous, ", {value}")?;
                    } else {
                        acc.insert(name.to_string(), value.to_string());
                    }
                    Ok::<_, anyhow::Error>(acc)
                },
            )
            .unwrap_or_default()
        {
            request = request.header(header, value);
        }

        // Body of the request
        let body = if let Some(data) = initialized.data.as_ref() {
            Body::from(tokio::fs::read(data).await?)
        } else {
            Body::empty()
        };

        let request = request.body(body)?;
        let response = context
            .request(request, uri.scheme_str() == Some("https"))
            .await?;

        context
            .transient_mut()
            .await
            .put_resource(response, ResourceKey::root());

        Ok(())
    }
}
