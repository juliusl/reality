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
use tracing::warn;

/// Type-alias for a secure client,
///
type SecureClient = hyper::Client<hyper_tls::HttpsConnector<HttpConnector>>;

pub fn secure_client() -> SecureClient {
    hyper::Client::builder().build(hyper_tls::HttpsConnector::new())
}

/// Type-alias for a local client,
///
type LocalClient = hyper::Client<HttpConnector>;

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
        if let Some(client) = $source.resource::<$client>(None) {
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
            .take_resource::<Response<Body>>(None)
            .map(|r| *r)
    }

    /// Registers an internal host alias,
    ///
    /// When the scheme/host of the alias uri is received, the scheme/host of the replacement will be applied instead.
    ///
    async fn register_internal_host_alias(&mut self, alias: Uri, replace: Uri) {
        let key = (alias.scheme(), alias.host());

        let value = (
            replace.scheme().cloned(),
            replace.host().map(|s| s.to_string()),
            replace.port_u16(),
        );

        if let Some(mut transport) =
            unsafe { self.host_mut(alias.scheme_str().unwrap_or_default()).await }
        {
            let key = ResourceKey::<(Option<Scheme>, Option<String>, Option<u16>)>::with_hash(key);
            transport.put_resource(value, Some(key));
        }
    }

    /// Lookup an alias for a host internall w/ the scheme/host of a uri being resolved,
    ///
    /// If successful, will return a a uri w/ the scheme and host replaced.
    ///
    async fn internal_host_lookup(&mut self, resolve: &Uri) -> Option<Uri> {
        let key = (resolve.scheme(), resolve.host());
        let key = ResourceKey::with_hash(key);

        let transport = self.host(resolve.scheme_str().unwrap_or_default()).await;
        let alias = transport.and_then(|t| {
            t.resource::<(Option<Scheme>, Option<String>, Option<u16>)>(Some(key))
                .as_deref()
                .cloned()
        });
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
    }
}

#[derive(Reality, Default, Debug, Clone)]
#[reality(plugin, rename = "utility/loopio.hyper.request")]
pub struct Request {
    /// Uri to make request to,
    ///
    #[reality(derive_fromstr)]
    uri: Uri,
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
            .internal_host_lookup(&initialized.uri)
            .await
            .unwrap_or(initialized.uri.clone());

        // println!("Resolved uri {:?}", uri);
        let mut request = hyper::Request::builder().uri(&uri);

        // Handle setting method on request
        if let Some(method) = Method::from_str(&initialized.method).ok() {
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

        context.transient_mut().await.put_resource(response, None);

        Ok(())
    }
}
