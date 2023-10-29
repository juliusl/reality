use std::collections::BTreeMap;

use async_trait::async_trait;
use poem::delete;
use poem::get;
use poem::head;
use poem::http::HeaderMap;
use poem::http::*;
use poem::listener::TcpListener;
use poem::patch;
use poem::post;
use poem::put;
use poem::web::Data;
use poem::web::Path;
use poem::Body;
use poem::EndpointExt;
use poem::FromRequest;
use poem::RequestBody;
use poem::ResponseParts;
use poem::Route;
use reality::prelude::*;
use tracing::error;

use crate::operation::Operation;
use crate::prelude::HyperExt;
use crate::ext::*;

/// Provides helper functions for accessing poem request resources,
///
pub trait PoemExt {
    /// Get path vars from storage,
    ///
    fn get_path_vars(&mut self) -> Option<PathVars>;

    /// Take the request body from storage,
    ///
    fn take_body(&mut self) -> Option<poem::Body>;

    /// Take headers from storage,
    ///
    fn take_response_parts(&mut self) -> Option<ResponseParts>;

    /// Set the status code on the response,
    ///
    fn set_status_code(&mut self, code: StatusCode);

    /// Sets a header on the response,
    ///
    fn set_header(
        &mut self,
        header: impl Into<HeaderName> + Send + Sync + 'static,
        value: impl Into<HeaderValue> + Send + Sync + 'static,
    );

    /// Sets the body on the response,
    ///
    fn set_response_body(&mut self, body: Body);

    /// Replaces the header map,
    ///
    fn replace_header_map(&mut self, header_map: HeaderMap);
}

impl PoemExt for ThunkContext {
    fn take_response_parts(&mut self) -> Option<ResponseParts> {
        let transient = self.transient();
        transient
            .storage
            .try_write()
            .ok()
            .and_then(|mut s| s.take_resource::<ResponseParts>(None).map(|b| *b))
    }

    fn take_body(&mut self) -> Option<poem::Body> {
        let transient = self.transient();
        transient
            .storage
            .try_write()
            .ok()
            .and_then(|mut s| s.take_resource::<Body>(None).map(|b| *b))
    }

    fn set_status_code(&mut self, code: StatusCode) {
        let transient = self.transient().storage;
        let transient = transient.try_write();

        if let Ok(mut transient) = transient {
            borrow_mut!(transient, ResponseParts, |parts| => {
                parts.status = code;
            });
        } else {
            error!("Could not write to transient storage. Existing read-lock.");
        }
    }

    fn set_header(
        &mut self,
        header: impl Into<HeaderName> + Send + Sync + 'static,
        value: impl Into<HeaderValue> + Send + Sync + 'static,
    ) {
        let transient = self.transient().storage;
        let transient = transient.try_write();

        if let Ok(mut transient) = transient {
            borrow_mut!(transient, ResponseParts, |parts| => {
                parts.headers.insert(header.into(), value.into());
            });
        } else {
            error!("Could not write to transient storage. Existing read-lock.");
        }
    }

    fn set_response_body(&mut self, body: Body) {
        let transient = self.transient().storage;
        let transient = transient.try_write();

        if let Ok(mut transient) = transient {
            transient.put_resource(body, Some(ResourceKey::with_hash("response")))
        } else {
            error!("Could not write to transient storage. Existing read-lock.");
        }
    }

    fn replace_header_map(&mut self, header_map: HeaderMap) {
        let transient = self.transient().storage;
        let transient = transient.try_write();

        if let Ok(mut transient) = transient {
            transient.put_resource(header_map, None)
        } else {
            error!("Could not write to transient storage. Existing read-lock.");
        }
    }

    fn get_path_vars(&mut self) -> Option<PathVars> {
        let transient = self.transient().storage;
        let transient = transient.try_read();

        if let Ok(transient) = transient {
            transient.resource::<PathVars>(None).as_deref().cloned()
        } else {
            error!("Could not write to transient storage. Existing read-lock.");
            None
        }
    }
}

/// Engine Proxy server plugin,
///
/// Routes requests to a specific engine operation,
///
#[derive(Reality, Default)]
#[reality(rename = "utility/loopio.poem.engine-proxy")]
pub struct EngineProxy {
    /// Address to host the proxy on,
    ///
    #[reality(derive_fromstr)]
    address: String,
    /// Adds a route for a HEAD request,
    ///
    #[reality(vec_of=Tagged<String>)]
    head: Vec<Tagged<String>>,
    /// Adds a route for a GET request,
    ///
    #[reality(vec_of=Tagged<String>)]
    get: Vec<Tagged<String>>,
    /// Adds a route for a POST request,
    ///
    #[reality(vec_of=Tagged<String>)]
    post: Vec<Tagged<String>>,
    /// Adds a route for a PUT request,
    ///
    #[reality(vec_of=Tagged<String>)]
    put: Vec<Tagged<String>>,
    /// Adds a route for a DELETE request,
    ///
    #[reality(vec_of=Tagged<String>)]
    delete: Vec<Tagged<String>>,
    /// Adds a route for a PATCH request,
    ///
    #[reality(vec_of=Tagged<String>)]
    patch: Vec<Tagged<String>>,
    /// Map of routes to the operations they map to,
    ///
    #[reality(map_of=String)]
    route: BTreeMap<String, String>,
    /// Map of routes to fold into the proxy route,
    ///
    #[reality(ignore)]
    routes: BTreeMap<String, poem::RouteMethod>,
}

impl Clone for EngineProxy {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            head: self.head.clone(),
            get: self.get.clone(),
            post: self.post.clone(),
            put: self.put.clone(),
            delete: self.delete.clone(),
            patch: self.patch.clone(),
            route: self.route.clone(),
            routes: BTreeMap::new(),
        }
    }
}

/// Type-alias for parsed path variable from a request,
///
pub type PathVars = Path<BTreeMap<String, String>>;

#[poem::handler]
async fn on_proxy(
    req: &poem::Request,
    mut body: Body,
    operation: Data<&Operation>,
) -> poem::Result<poem::Response> {
    let path_vars = PathVars::from_request(req, &mut RequestBody::new(body)).await?;

    let mut operation = operation.0.clone();
    if let Some(context) = operation.context_mut() {
        let mut storage = context.transient.storage.write().await;
        storage.put_resource(path_vars, None);
    }

    let mut context = operation
        .execute()
        .await
        .map_err(|e| poem::Error::from_string(format!("{e}"), StatusCode::INTERNAL_SERVER_ERROR))?;

    #[cfg(feature = "hyper-ext")]
    match context.take_response().await {
        Some(response) => Ok(response.into()),
        None => Ok(poem::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .finish()),
    }

    #[cfg(not(feature = "hyper-ext"))]
    match (context.take_response_parts(), context.take_body()) {
        (Some(parts), Some(body)) => Ok(poem::Response::from_parts(parts, body)),
        _ => Ok(poem::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .finish()),
    }
}

macro_rules! create_routes {
    ($ctx:ident, $rcv:ident, [$($ident:tt),*]) => {
        $(
            for (value, tag) in $rcv.$ident.iter().map(|g| (g.value(), g.tag())) {
                match (value, tag) {
                    (Some(route), Some(op)) => {
                        if let Some(operation) = $ctx
                            .engine_handle()
                            .clone()
                            .and_then(|e| e.operations.get(op).cloned())
                        {
                            if let Some(_route) = $rcv.routes.remove(route) {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), _route.$ident(on_proxy.data(operation)));
                            } else {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), $ident(on_proxy.data(operation)));
                            }
                        }
                    }
                    _ => {}
                }
            }
        )*

    };
}

#[async_trait]
impl CallAsync for EngineProxy {
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        let mut initialized = context.initialized::<EngineProxy>().await;
        assert!(
            initialized.routes.is_empty(),
            "Routes should only be initialized when the plugin is being run"
        );

        // Build routes for proxy server
        create_routes!(context, initialized, [head, get, post, put, patch, delete]);

        let route = initialized
            .routes
            .into_iter()
            .fold(Route::new(), |acc, (route, route_method)| {
                acc.at(route, route_method)
            });

        poem::Server::new(TcpListener::bind(initialized.address))
            .run_with_graceful_shutdown(route, context.cancellation.clone().cancelled(), None)
            .await?;

        Ok(())
    }
}
