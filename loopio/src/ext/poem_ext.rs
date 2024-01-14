use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use poem::get;
use poem::http::HeaderMap;
use poem::http::*;
use poem::listener::Acceptor;
use poem::listener::Listener;
use poem::listener::TcpListener;
use poem::web::Data;
use poem::web::Path;
use poem::Body;
use poem::Endpoint;
use poem::EndpointExt;
use poem::FromRequest;
use poem::IntoEndpoint;
use poem::RequestBody;
use poem::ResponseParts;
use poem::Route;
use poem::RouteMethod;
use reality::prelude::*;
use reality::CommaSeperatedStrings;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::action::ActionExt;
use crate::ext::*;
use crate::prelude::Action;
use crate::prelude::Address;
use crate::prelude::HyperExt;
use crate::prelude::UriParam;

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

    /// Take body and response parts from transient target splitting for re-constructing a response,
    ///
    async fn split_for_response(&mut self) -> Option<(ResponseParts, poem::Body)>;

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
    async fn split_for_response(&mut self) -> Option<(ResponseParts, poem::Body)> {
        if let Some(target) = self.reset() {
            let mut target = target.storage.write().await;

            let mut root = target.root();

            match (root.take::<ResponseParts>(), root.take::<poem::Body>()) {
                (Some(parts), Some(body)) => Some((*parts, *body)),
                _ => None,
            }
        } else {
            None
        }
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
            .put_resource(body, ResourceKey::with_hash("response"))
    }

    #[inline]
    async fn replace_header_map(&mut self, header_map: HeaderMap) {
        self.transient_mut().await.root().put(header_map)
    }

    #[inline]
    async fn get_path_vars(&mut self) -> Option<PathVars> {
        self.transient().await.root_ref().current()
    }

    #[inline]
    async fn take_request(&self) -> Option<PoemRequest> {
        self.transient_mut().await.root().take().map(|r| *r)
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
#[derive(Reality, Serialize, Deserialize, PartialEq, PartialOrd, Default)]
#[plugin_def(
    call = start_engine_proxy
)]
#[parse_def(group = "builtin", rename = "engine-proxy")]
pub struct EngineProxy {
    /// Address to host the proxy on,
    ///
    #[reality(derive_fromstr)]
    address: String,
    /// If set, maps this alias to the address of this proxy
    ///
    #[reality(option_of=String)]
    alias: Option<String>,
    /// Map of routes to the operations they map to,
    ///
    #[reality(vec_of=Decorated<Address>)]
    route: Vec<Decorated<Address>>,
    #[reality(ignore)]
    #[serde(skip)]
    routes: Vec<RouteConfig>,
}

#[derive(Clone)]
pub struct RouteConfig {
    path: String,
    resource: HostedResource,
    methods: Option<Delimitted<',', String>>,
}

impl PartialEq for RouteConfig {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.methods == other.methods
    }
}

impl PartialOrd for RouteConfig {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.path.partial_cmp(&other.path) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.methods.partial_cmp(&other.methods)
    }
}

impl RouteConfig {
    /// Configures a poem route w/ an endpoint,
    ///
    pub fn configure_route<E: Endpoint + 'static>(
        &self,
        route: Route,
        endpoint: impl Fn() -> E,
    ) -> Route {
        let ep = move |resource: &HostedResource| endpoint().data(resource.clone());

        let resource = &self.resource;

        if let Some(methods) = self.methods.as_ref() {
            // Parse the methods from decoration properties
            let methods = methods
                .clone()
                .into_iter()
                .filter_map(|m| Method::from_str(&m).ok());
            let route_method = methods.fold(RouteMethod::new(), move |route, m| match m {
                Method::GET => route.get(ep(resource)),
                Method::HEAD => route.head(ep(resource)),
                Method::OPTIONS => route.options(ep(resource)),
                Method::PUT => route.put(ep(resource)),
                Method::POST => route.post(ep(resource)),
                Method::PATCH => route.patch(ep(resource)),
                Method::DELETE => route.delete(ep(resource)),
                Method::CONNECT => route.connect(ep(resource)),
                Method::TRACE => route.trace(ep(resource)),
                _ => route,
            });
            debug!("Adding route {}", self.path);
            route.at(self.path.to_string(), route_method)
        } else {
            debug!("Adding route {}", self.path);
            route.at(self.path.to_string(), get(ep(resource)))
        }
    }
}

/// Starts the engine proxy,
///
async fn start_engine_proxy(context: &mut ThunkContext) -> anyhow::Result<()> {
    let initialized = Remote.create::<EngineProxy>(context).await;

    debug!("Starting {:?}", initialized);
    // Find hosted resources to route to
    let mut resources = BTreeMap::new();
    for address in initialized.route.iter().filter_map(|v| v.value()) {
        if let Some(eh) = context.engine_handle().await {
            match eh.hosted_resource(address.to_string()).await {
                Ok(resource) => {
                    trace!("Adding hosted resource for setting - {address}");
                    resources.insert(address, resource);
                }
                Err(err) => {
                    error!("Could not find hosted resource -- {err}");
                }
            }
        } else {
            error!("Did not have engine handle");
        }
    }

    let route_config = initialized
        .route
        .iter()
        .fold(vec![], |mut config_collection, route| {
            let path = route
                .property("path")
                .or(route.value().map(|r| r.to_string()));

            let config = RouteConfig {
                path: path.expect("should have a path value").to_string(),
                resource: resources
                    .get(&route.value().unwrap())
                    .cloned()
                    .unwrap_or_default(),
                methods: route
                    .property("methods")
                    .and_then(|m| CommaSeperatedStrings::from_str(m.as_str()).ok()),
            };
            config_collection.push(config);
            config_collection
        });

    // Update the route_config setting
    let (attr, routes) = (context.attribute, route_config.clone());
    context.node().await.lazy_dispatch_mut(move |s| {
        if let Some(mut proxy) = s.entry(attr).get_mut::<EngineProxy>() {
            proxy.routes = routes;
        }
    });

    context.process_node_updates().await;

    // Create route handler
    let route = route_config.iter().fold(Route::new(), |route, config| {
        config.configure_route(route, || on_proxy)
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
            .path_and_query(format!(
                "/?engine-proxy={}",
                attr.address().unwrap_or_default()
            ))
            .build()?;
        let alias = Uri::builder()
            .scheme(scheme.clone())
            .authority(alias)
            .path_and_query("/")
            .build()?;

        eprintln!("Adding alias: {:?} -> {:?}", alias, replace_with);

        context
            .notify(Some(Bytes::copy_from_slice(
                replace_with.to_string().as_bytes(),
            )))
            .await?;
    }

    eprintln!(
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

impl Debug for EngineProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineProxy")
            .field("address", &self.address)
            .field("alias", &self.alias)
            .field("route", &self.route)
            .finish()
    }
}

impl Clone for EngineProxy {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            alias: self.alias.clone(),
            route: self.route.clone(),
            routes: self.routes.clone(),
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
    operation: Data<&HostedResource>,
) -> poem::Result<poem::Response> {
    let mut body = RequestBody::new(body);
    let path_vars = PathVars::from_request(req, &mut body).await?;

    let mut resource = operation.clone();
    if let Some(_previous) = resource.context_mut().reset() {
        warn!("Previous transient target detected");
    }

    resource
        .context_mut()
        .transient_mut()
        .await
        .root()
        .put(PoemRequest {
            path: path_vars,
            uri: req.uri().clone(),
            headers: req.headers().clone(),
            body: Some(body),
        });

    if let CallOutput::Spawn(Some(spawned)) = resource.spawn() {
        match spawned.await.map_err(|_| {
            poem::Error::from_string(
                "Hosted resource is unresponsive",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        })? {
            Ok(mut context) => {
                #[cfg(feature = "hyper-ext")]
                match context.take_response().await {
                    Some(response) => Ok(response.into()),
                    None => Ok(poem::Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .finish()),
                }

                #[cfg(not(feature = "hyper-ext"))]
                if let Some((parts, body)) = context.split_for_response().await {
                    Ok(poem::Response::from_parts(parts, body))
                } else {
                    Ok(poem::Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .finish())
                }
            }
            Err(err) => Err(poem::Error::from_string(
                format!("{err}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            )),
        }
    } else {
        Ok(poem::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .finish())
    }
}

/// Reverse proxy config,
///
#[derive(Reality, Serialize, Deserialize, Clone, PartialEq, Default)]
#[reality(plugin, call = configure_reverse_proxy, rename = "reverse-proxy-config", group = "builtin")]
pub struct ReverseProxyConfig {
    /// Alias this config is for,
    ///
    #[reality(derive_fromstr)]
    alias: UriParam,
    /// Allow headers,
    ///
    #[reality(rename = "allow-headers", option_of=CommaSeperatedStrings)]
    allow_headers: Option<CommaSeperatedStrings>,
    /// Deny headers,
    ///
    #[reality(rename = "deny-headers", option_of=CommaSeperatedStrings)]
    deny_headers: Option<CommaSeperatedStrings>,
    /// Hosts to allow,
    ///
    #[reality(rename = "allow-hosts", option_of=CommaSeperatedStrings)]
    allow_hosts: Option<CommaSeperatedStrings>,
}

impl ReverseProxyConfig {
    /// Configure an endpoint w/ reverse proxy settings,
    ///
    pub fn decorate(&self, endpoint: impl IntoEndpoint + Endpoint) -> impl IntoEndpoint + Endpoint {
        let allow_headers = self.allow_headers.clone();
        let deny_headers = self.deny_headers.clone();
        let allow_hosts = self.allow_hosts.clone();
        endpoint.before(move |mut req| {
            let mut result = Ok(());
            let host = req.header("host").unwrap_or_default().to_string();

            if let Some(deny_headers) = &deny_headers {
                for h in deny_headers.clone() {
                    req.headers_mut().remove(h);
                }
            }

            if let Some(allow_headers) = &allow_headers {
                let mut headers = HeaderMap::new();

                for h in allow_headers.clone() {
                    if let Some(v) = req.header(&h) {
                        if let (Ok(header), Ok(value)) =
                            (HeaderName::from_str(&h), HeaderValue::from_str(v))
                        {
                            headers.insert(header, value);
                        }
                    }
                }
                *req.headers_mut() = headers;
                if let Ok(host) = HeaderValue::from_str(&host) {
                    req.headers_mut().insert("host", host);
                }
            }

            if let Some(allow_hosts) = &allow_hosts {
                if !allow_hosts
                    .clone()
                    .fold(false, |allow, h| allow | (host == h))
                {
                    result = Err(poem::Error::from_string(
                        "Host is not allowed",
                        StatusCode::FORBIDDEN,
                    ));
                }
            }

            async {
                result?;
                Ok(req)
            }
        })
    }
}

/// Reverse proxy plugin,
///
#[derive(Reality, Serialize, Deserialize, PartialEq, Default)]
#[reality(plugin, call = start_reverse_proxy, rename = "reverse-proxy", group = "builtin")]
pub struct ReverseProxy {
    /// Address to host the proxy on,
    ///
    #[reality(derive_fromstr)]
    address: String,
    /// Forward request to this host,
    ///
    #[reality(vec_of=UriParam)]
    forward: Vec<UriParam>,
}

async fn start_reverse_proxy(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.as_remote_plugin::<ReverseProxy>().await;

    let mut route = Route::new();

    let client = Arc::new(hyper_ext::local_client());

    for host in init.forward.iter() {
        let key = ResourceKey::with_hash(host.as_ref().to_string());
        let mut transient = tc.transient_mut().await;
        let entry = transient.entry(key);

        if let (Some(rp_config), Some(routes), Some(internal_host)) = (
            entry.get::<ReverseProxyConfig>(),
            entry.get::<Vec<RouteConfig>>(),
            entry.get::<Arc<Uri>>(),
        ) {
            route = routes.iter().fold(Route::new(), |route, config| {
                config.configure_route(route, || {
                    rp_config.decorate(
                        on_forward_request
                            .data(client.clone())
                            .data(internal_host.clone()),
                    )
                })
            });
        };
    }

    let listener = TcpListener::bind(&init.address);
    eprintln!("Listening to {}", init.address);

    poem::Server::new(listener)
        .run_with_graceful_shutdown(route, tc.cancellation.child_token().cancelled(), None)
        .await?;

    Ok(())
}

impl Clone for ReverseProxy {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            forward: self.forward.clone(),
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
    eprintln!("{:?}", uri);
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

    if let Some((_, event_message)) = tc.fetch_kv::<Bytes>("inbound_event_message") {
        debug!("Got event_message {:?}", event_message);
        if let Ok(internal_host) = std::str::from_utf8(&event_message)
            .map_err(|e| anyhow!(e))
            .and_then(|s| Uri::from_str(s).map_err(|e| anyhow!(e)))
        {
            debug!("Parsed event_message {:?}", internal_host);

            let address = internal_host
                .query()
                .map(|q| q.trim_start_matches("engine-proxy=").to_string());
            let internal_host = Arc::new(
                format!(
                    "http://localhost:{}",
                    internal_host.port_u16().expect("should have a port")
                )
                .parse::<Uri>()?,
            );

            if let Some(engine_proxy) = tc
                .engine_handle()
                .await
                .expect("should be bound to an engine")
                .hosted_resource(address.unwrap())
                .await
                .ok()
            {
                let engine_proxy = engine_proxy.context().initialized::<EngineProxy>().await;
                debug!(
                    "Getting route config from engine proxy for {}\n{:#?}",
                    init.alias.as_ref(),
                    engine_proxy
                );
                let mut transient = tc.transient_mut().await;
                let mut entry = transient.entry(ResourceKey::with_hash(init.alias.to_string()));
                entry.put(engine_proxy.routes.clone());
                entry.put(init.clone());
                entry.put(internal_host.clone());
            }
            debug!(
                "Configured reverse proxy for {:?} -> {internal_host}",
                init.alias
            );
        }
    }

    Ok(())
}
