use async_trait::async_trait;
use hyper::client::HttpConnector;
use reality::prelude::*;

use crate::plugin::ThunkContext;

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
}
