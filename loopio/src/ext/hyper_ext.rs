use std::collections::BTreeMap;
use std::path::PathBuf;

use async_trait::async_trait;
use hyper::Body;
use hyper::Method;
use hyper::Response;
use hyper::Uri;
use hyper::client::HttpConnector;
use reality::prelude::*;

/// Type-alias for a secure client,
///
type SecureClient = hyper::Client<hyper_tls::HttpsConnector<HttpConnector>>;

/// Type-alias for a local client,
///
type LocalClient = hyper::Client<HttpConnector>;

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
        let source = self.source().await;

        if use_https {
            do_request!(source, request, SecureClient)
        } else {
            do_request!(source, request, LocalClient)
        }
    }

    async fn take_response(&mut self) -> Option<hyper::Response<hyper::Body>> {
       let mut storage = self.transient.storage.write().await;

       storage.take_resource::<Response<Body>>(None).map(|r| *r)
    }
}

#[derive(Reality, Default, Clone)]
#[reality(rename = "utility/loopio.hyper.request")]
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
        let mut request = hyper::Request::builder().uri(initialized.uri.clone());

        // Handle setting method on request
        if let Some(method) = Method::from_str(&initialized.method).ok() {
            request = request.method(method);
        }

        // Handle adding headers
        for (header, value) in initialized
            .headers
            .iter()
            .try_fold(BTreeMap::<String, String>::new(), |mut acc, (name, value)| {
                if let Some(previous) = acc.get_mut(name) {
                    use std::fmt::Write;
                    write!(previous, ", {value}")?;
                } else {
                    acc.insert(name.to_string(), value.to_string());
                }
                Ok::<_, anyhow::Error>(acc)
            })
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
            .request(request, initialized.uri.scheme_str() == Some("https"))
            .await?;

        let mut transient = context.transient.storage.write().await;
        transient.put_resource(response, None);

        Ok(())
    }
}
