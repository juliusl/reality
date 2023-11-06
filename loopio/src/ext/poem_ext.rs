use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

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
    async fn get_path_vars(&mut self) -> Option<PathVars>;

    /// Take the request body from storage,
    ///
    async fn take_body(&mut self) -> Option<poem::Body>;

    /// Take headers from storage,
    ///
    async fn take_response_parts(&mut self) -> Option<ResponseParts>;

    /// Set the status code on the response,
    ///
    async fn set_status_code(&mut self, code: StatusCode);

    /// Sets a header on the response,
    ///
    async fn set_header(
        &mut self,
        header: impl Into<HeaderName> + Send + Sync + 'static,
        value: impl Into<HeaderValue> + Send + Sync + 'static,
    );

    /// Sets the body on the response,
    ///
    async fn set_response_body(&mut self, body: Body);

    /// Replaces the header map,
    ///
    async fn replace_header_map(&mut self, header_map: HeaderMap);

    /// Take request from the current storage target if one exists,
    ///
    async fn take_request(&self) -> Option<PoemRequest>;

    /// Scans the current node for any reverse proxy configs,
    ///
    async fn scan_for_reverse_proxy_config(&self) -> Vec<ReverseProxyConfig>;
}

#[async_trait]
impl PoemExt for ThunkContext {
    #[inline]
    async fn take_response_parts(&mut self) -> Option<ResponseParts> {
        self.transient_mut()
            .await
            .take_resource::<ResponseParts>(None)
            .map(|b| *b)
    }

    #[inline]
    async fn take_body(&mut self) -> Option<poem::Body> {
        self.transient_mut()
            .await
            .take_resource::<poem::Body>(None)
            .map(|b| *b)
    }

    #[inline]
    async fn set_status_code(&mut self, code: StatusCode) {
        let mut transient = self.transient_mut().await;

        borrow_mut!(transient, ResponseParts, |parts| => {
            parts.status = code;
        });
    }

    #[inline]
    async fn set_header(
        &mut self,
        header: impl Into<HeaderName> + Send + Sync + 'static,
        value: impl Into<HeaderValue> + Send + Sync + 'static,
    ) {
        let mut transient = self.transient_mut().await;

        borrow_mut!(transient, ResponseParts, |parts| => {
            parts.headers.insert(header.into(), value.into());
        });
    }

    #[inline]
    async fn set_response_body(&mut self, body: Body) {
        self.transient_mut()
            .await
            .put_resource(body, Some(ResourceKey::with_hash("response")))
    }

    #[inline]
    async fn replace_header_map(&mut self, header_map: HeaderMap) {
        self.transient_mut().await.put_resource(header_map, None)
    }

    #[inline]
    async fn get_path_vars(&mut self) -> Option<PathVars> {
        self.transient().await.current_resource::<PathVars>(None)
    }

    #[inline]
    async fn take_request(&self) -> Option<PoemRequest> {
        self.transient_mut()
            .await
            .take_resource::<PoemRequest>(None)
            .map(|r| *r)
    }

    #[inline]
    async fn scan_for_reverse_proxy_config(&self) -> Vec<ReverseProxyConfig> {
        self.scan_node().await
    }
}

/// Engine Proxy server plugin,
///
/// Routes requests to a specific engine operation,
///
#[derive(Reality, Default)]
#[reality(plugin, call = start_engine_proxy, rename = "utility/loopio.poem.engine-proxy")]
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

                context.transient_mut().await.put_resource(
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
    ($handler:ident, $ctx:ident, $rcv:ident, [$($ident:tt),*]) => {
        let engine_handle = $ctx.engine_handle().await;
        assert!(engine_handle.is_some());
        let engine_handle = engine_handle.unwrap();
        let operations = engine_handle.operations.clone();
        let sequences = engine_handle.sequences.clone();

        $(
            for (value, tag) in $rcv.$ident.iter().map(|g| (g.value(), g.tag())) {
                match (value, tag) {
                    (Some(route), Some(op)) => {
                        let op = $rcv.route.get(op).cloned().unwrap_or_default();
                        if let Some(operation) = operations.get(&op).cloned()
                        {
                            if let Some(_route) = $rcv.routes.remove(route) {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), _route.$ident($handler.data(Either::<Operation, Sequence>::Left(operation))));
                            } else {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), $ident($handler.data(Either::<Operation, Sequence>::Left(operation))));
                            }
                        } else if let Some(sequence) = sequences.get(&op).cloned() {
                            if let Some(_route) = $rcv.routes.remove(route) {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), _route.$ident($handler.data(Either::<Operation, Sequence>::Right(sequence))));
                            } else {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), $ident($handler.data(Either::<Operation, Sequence>::Right(sequence))));
                            }
                        }
                    }
                    _ => {}
                }
            }
        )*
    };
    ($handler:expr, $ctx:ident, $rcv:ident, [$($ident:tt),*]) => {
        let engine_handle = $ctx.engine_handle().await;
        assert!(engine_handle.is_some());
        let engine_handle = engine_handle.unwrap();
        let operations = engine_handle.operations.clone();
        let sequences = engine_handle.sequences.clone();

        $(
            for (value, tag) in $rcv.$ident.iter().map(|g| (g.value(), g.tag())) {
                match (value, tag) {
                    (Some(route), Some(op)) => {
                        let op = $rcv.route.get(op).cloned().unwrap_or_default();
                        if let Some(_) = operations.get(&op).cloned()
                        {
                            if let Some(_route) = $rcv.routes.remove(route) {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), _route.$ident($handler()));
                            } else {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), $ident($handler()));
                            }
                        } else if let Some(_) = sequences.get(&op).cloned() {
                            if let Some(_route) = $rcv.routes.remove(route) {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), _route.$ident($handler()));
                            } else {
                                $rcv
                                    .routes
                                    .insert(route.to_string(), $ident($handler()));
                            }
                        }
                    }
                    _ => {}
                }
            }
        )*
    };
}

/// Starts the engine proxy,
///
async fn start_engine_proxy(context: &mut ThunkContext) -> anyhow::Result<()> {
    let mut initialized = context.initialized::<EngineProxy>().await;
    assert!(
        initialized.routes.is_empty(),
        "Routes should only be initialized when the plugin is being run"
    );

    // Build routes for proxy server
    create_routes!(
        on_proxy,
        context,
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
            .scheme(scheme.clone())
            .authority(alias)
            .path_and_query("/")
            .build();

        println!("Adding alias: {:?} -> {:?}", alias, replace_with);
        context
            .register_internal_host_alias(alias?, replace_with?)
            .await;

        context.notify_host(
            scheme.as_str(), 
            "engine_proxy_started"
        ).await?;
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

#[poem::async_trait]
impl<'a, T: FieldPacketType> FromRequest<'a> for P<T> {
    async fn from_request(_: &'a poem::Request, _: &mut RequestBody) -> poem::Result<Self> {
        Ok(Self(PhantomData))
    }
}

struct P<T>(pub PhantomData<T>);

#[handler]
async fn handle_remote_frame<T: ApplyFrame + Clone + Send + Sync + 'static>(
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
                .apply_frame(packets)
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

/// Reverse proxy config,
///
#[derive(Reality, Clone, Default)]
#[reality(plugin, call = configure_reverse_proxy, rename = "utility/loopio.poem.reverse-proxy-config")]
pub struct ReverseProxyConfig {
    /// Alias this config is for,
    ///
    #[reality(derive_fromstr)]
    alias: Uri,
    /// Allow headers,
    ///
    #[reality(rename = "allow-headers")]
    allow_headers: String,
    /// Deny headers,
    ///
    #[reality(rename = "deny-headers")]
    deny_headers: String,
    /// Hosts to allow,
    ///
    #[reality(rename = "allow-hosts")]
    allow_hosts: String,
}

/// Reverse proxy plugin,
///
#[derive(Reality, Default)]
#[reality(plugin, call = start_reverse_proxy, rename = "utility/loopio.poem.reverse-proxy")]
pub struct ReverseProxy {
    /// Address to host the proxy on,
    ///
    #[reality(derive_fromstr)]
    address: String,
    /// Hosts to start the proxy w/,
    ///
    #[reality(vec_of=Uri)]
    host: Vec<Uri>,
}

async fn start_reverse_proxy(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<ReverseProxy>().await;

    let mut routes = BTreeMap::new();

    for host in init.host.iter() {
        let mut transient = tc.transient_mut().await;
        let resource = transient.take_resource::<EngineProxy>(Some(ResourceKey::with_hash(host.to_string())));
        println!(
            "Processing reverse proxy config for {}",
            host,
        );
        if let Some(resource) = resource {
            for (address, route_method) in (*resource).routes {
                println!("Forwarding route {}", address);
                routes.insert(address, route_method);
            }
        }
        // for (address, route_method) in resource.iter() {
        //     println!("Forwarding route {}", address);
        //     routes.insert(address.clone(), route_method.clone());
        // }
    }

    let mut route = Route::new();
    for (address, _route) in routes {
        route = route.at(address, _route);
    }

    let listener = TcpListener::bind(&init.address);
    println!("Listening to {}", init.address);

    poem::Server::new(listener)
        .run_with_graceful_shutdown(route, tc.cancellation.child_token().cancelled(), None)
        .await?;

    Ok(())
}

impl Clone for ReverseProxy {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            host: self.host.clone(),
        }
    }
}

#[poem::handler]
async fn on_forward_request(
    req: &poem::Request,
    body: Body,
    local_client: Data<&Arc<crate::ext::hyper_ext::LocalClient>>,
    internal_host: Data<&Arc<Uri>>,
) -> poem::Result<poem::Response> {
    let local_client = local_client.0.clone();

    let mut headers = req.headers();
    let mut forward_req = Request::builder();
    if let Some(_headers) = forward_req.headers_mut() {
        for (h, v) in headers.iter() {
            _headers.append(h, v.clone());
        }
    }

    let uri = internal_host.clone();
    if let (Some(scheme), Some(host), Some(port)) = (uri.scheme(), uri.host(), uri.port_u16()) {
        let forwarding = Uri::builder()
            .scheme(scheme.clone())
            .authority(format!("{}:{}", host, port))
            .path_and_query(
                req.uri()
                    .path_and_query()
                    .map(|p| p.as_str())
                    .unwrap_or(req.uri().path()),
            );

        local_client
            .request(
                forward_req
                    .uri(
                        forwarding
                            .build()
                            .map_err(|e| poem::Error::new(e, StatusCode::SERVICE_UNAVAILABLE))?,
                    )
                    .body(body.into())
                    .map_err(|e| poem::Error::new(e, StatusCode::SERVICE_UNAVAILABLE))?,
            )
            .await
            .map(|r| r.into())
            .map_err(|e| poem::Error::new(e, StatusCode::SERVICE_UNAVAILABLE))
    } else {
        Err(poem::Error::from_string(
            "Unknown route",
            StatusCode::NOT_FOUND,
        ))
    }
}

/// Configures the reverse proxy,
///
async fn configure_reverse_proxy(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<ReverseProxyConfig>().await;

    if let (Some(host), Some(internal_host)) = (
        init.alias.scheme_str(),
        tc.internal_host_lookup(&init.alias).await,
    ) {
        let client = Arc::new(hyper_ext::local_client());
        let client = &client;

        let internal_host = Arc::new(internal_host);
        let internal_host = &internal_host;

        if let Some(mut engine_proxy) = tc.scan_host_for::<EngineProxy>(host).await {
            println!("Configuring reverse proxy for {}", init.alias);
            // TODO
            // 1)  apply allow/deny headers
            // 2)  apply allow/deny hosts
            // 3)
            create_routes!(
                move || {
                    on_forward_request
                        .data(client.clone())
                        .data(internal_host.clone())
                },
                tc,
                engine_proxy,
                [head, get, post, put, patch, delete]
            );

            tc.transient_mut().await.put_resource(
                engine_proxy,
                Some(ResourceKey::with_hash(init.alias.to_string())),
            );
        }
    }
    Ok(())
}
