use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Cursor;

use async_trait::async_trait;
use poem::delete;
use poem::get;
use poem::handler;
use poem::head;
use poem::http::HeaderMap;
use poem::http::*;
use poem::listener::Acceptor;
use poem::listener::Listener;
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
use poem::Response;
use poem::ResponseParts;
use poem::Route;
use reality::prelude::*;
use tokio_util::either::Either;
use tracing::error;

use crate::ext::*;
use crate::operation::Operation;
use crate::prelude::HyperExt;
use crate::sequence::Sequence;

pub struct PoemRequest {
    pub path: Path<BTreeMap<String, String>>,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Option<RequestBody>,
}

impl Clone for PoemRequest {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            uri: self.uri.clone(),
            headers: self.headers.clone(),
            body: None,
        }
    }
}

/// Provides helper functions for accessing poem request resources,
///
#[async_trait]
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

    /// Take request from the current storage target if one exists,
    ///
    fn take_request(&self) -> Option<PoemRequest>;
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
            .and_then(|mut s| s.take_resource::<poem::Body>(None).map(|b| *b))
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

    fn take_request(&self) -> Option<PoemRequest> {
        let transient = self.transient().storage;
        let transient = transient.try_write();

        if let Ok(mut transient) = transient {
            transient.take_resource::<PoemRequest>(None).map(|r| *r)
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
#[reality(plugin, rename = "utility/loopio.poem.engine-proxy")]
pub struct EngineProxy {
    /// Address to host the proxy on,
    ///
    #[reality(derive_fromstr)]
    address: String,
    /// If set, maps this alias to the address of this proxy
    /// 
    #[reality(option_of=String)]
    alias: Option<String>,
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

impl Debug for EngineProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineProxy")
            .field("address", &self.address)
            .field("head", &self.head)
            .field("get", &self.get)
            .field("post", &self.post)
            .field("put", &self.put)
            .field("delete", &self.delete)
            .field("patch", &self.patch)
            .field("route", &self.route)
            .finish()
    }
}

impl Clone for EngineProxy {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            alias: self.alias.clone(),
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
    operation: Data<&Either<Operation, Sequence>>,
) -> poem::Result<poem::Response> {
    let mut body = RequestBody::new(body);
    let path_vars = PathVars::from_request(req, &mut body).await?;

    match *operation {
        Either::Left(op) => {
            let mut operation = op.clone();
            if let Some(context) = operation.context_mut() {
                context.reset();
                
                let mut storage = context.transient.storage.write().await;
                storage.put_resource(
                    PoemRequest {
                        path: path_vars,
                        uri: req.uri().clone(),
                        headers: req.headers().clone(),
                        body: Some(body),
                    },
                    None,
                );
            }

            let mut context = operation.execute().await.map_err(|e| {
                poem::Error::from_string(format!("{e}"), StatusCode::INTERNAL_SERVER_ERROR)
            })?;

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
        Either::Right(_seq) => match _seq.clone().await {
            Ok(mut context) => {
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
            Err(err) => {
                error!("{err}");
                Ok(poem::Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .finish())
            }
        },
    }
}

macro_rules! create_routes {
    ($ctx:ident, $operations:ident, $sequences:ident, $rcv:ident, [$($ident:tt),*]) => {
        $(
            for (value, tag) in $rcv.$ident.iter().map(|g| (g.value(), g.tag())) {
                match (value, tag) {
                    (Some(route), Some(op)) => {
                        let op = $rcv.route.get(op).cloned().unwrap_or_default();
                        if let Some(operation) = $operations.get(&op).cloned()
                        {
                            if let Some(_route) = $rcv.routes.remove(route) {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), _route.$ident(on_proxy.data(Either::<Operation, Sequence>::Left(operation))));
                            } else {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), $ident(on_proxy.data(Either::<Operation, Sequence>::Left(operation))));
                            }
                        } else if let Some(sequence) = $sequences.get(&op).cloned() {
                            if let Some(_route) = $rcv.routes.remove(route) {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), _route.$ident(on_proxy.data(Either::<Operation, Sequence>::Right(sequence))));
                            } else {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), $ident(on_proxy.data(Either::<Operation, Sequence>::Right(sequence))));
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

        let engine_handle = context.engine_handle().clone();
        assert!(engine_handle.is_some());
        let engine_handle = engine_handle.unwrap();
        let operations = engine_handle.operations.clone();
        let sequences = engine_handle.sequences.clone();

        // Build routes for proxy server
        create_routes!(
            context,
            operations,
            sequences,
            initialized,
            [head, get, post, put, patch, delete]
        );

        let route = initialized
            .routes
            .into_iter()
            .fold(Route::new(), |acc, (route, route_method)| {
                acc.at(route, route_method)
            });

        let listener = TcpListener::bind(&initialized.address)
            .into_acceptor()
            .await?;

        // If `alias` is set, register the proxy to that alias
        if let (Some(addr), Some((Some(scheme), Some(alias)))) = (
            listener.local_addr().first(),
            initialized
                .alias
                .and_then(|a| a.parse::<Uri>().ok())
                .map(|u| (u.scheme().cloned(), u.host().map(|h| h.to_string()))),
        ) {
            let port = addr.0.as_socket_addr().unwrap().port();
            let replace_with = Uri::builder()
                .scheme("http")
                .authority(format!("localhost:{}", port))
                .path_and_query("/")
                .build();
            let alias = Uri::builder()
                .scheme(scheme)
                .authority(alias)
                .path_and_query("/")
                .build();

            println!("Adding alias: {:?} -> {:?}", alias, replace_with);
            context
                .register_internal_host_alias(alias?, replace_with?)
                .await;
        }

        println!(
            "listening on {:#?}",
            listener
                .local_addr()
                .iter()
                .map(|l| l.0.to_string())
                .collect::<Vec<_>>()
        );
        poem::Server::new_with_acceptor(listener)
            .run_with_graceful_shutdown(route, context.cancellation.child_token().cancelled(), None)
            .await?;

        Ok(())
    }
}

#[poem::async_trait]
impl<'a, T: FieldPacketType> FromRequest<'a> for P<T> {
    async fn from_request(_: &'a poem::Request, _: &mut RequestBody) -> poem::Result<Self> {
        Ok(Self(PhantomData))
    }
}

struct P<T>(pub PhantomData<T>);

#[handler]
async fn handle_remote_frame<T: FromFrame + Clone + Send + Sync + 'static>(
    initialized: Data<&T>,
    thunk_context: Data<&ThunkContext>,
    frame: poem::Body,
) -> poem::Result<Response> {
    let bytes = frame.into_bytes().await?;
    let packets = bincode::deserialize::<Vec<FieldPacket>>(&bytes);
    match packets {
        Ok(packets) => {
            let mut initialized = initialized.clone();
            initialized
                .from_frame(packets)
                .map_err(|e| poem::Error::from_string(format!("{e}"), StatusCode::BAD_REQUEST))?;
            let (_variant, branch) = thunk_context.branch();
            unsafe {
                let mut source = branch.node_mut().await;
                source.put_resource(initialized, branch.attribute.map(|t| t.transmute()));
            }
            let _result = branch.call().await.unwrap();
            // TODO -- _variant + _result + destinatio operation/sequence
        }
        Err(err) => {
            error!("{err}");
        }
    }
    Ok(Response::builder().finish())
}
