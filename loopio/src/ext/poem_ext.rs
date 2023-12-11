use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
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
use tracing::error;

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
            .take_resource::<ResponseParts>(ResourceKey::root())
            .map(|b| *b)
    }

    #[inline]
    async fn take_body(&mut self) -> Option<poem::Body> {
        self.transient_mut()
            .await
            .take_resource::<poem::Body>(ResourceKey::root())
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
            .put_resource(body, ResourceKey::with_hash("response"))
    }

    #[inline]
    async fn replace_header_map(&mut self, header_map: HeaderMap) {
        self.transient_mut()
            .await
            .put_resource(header_map, ResourceKey::root())
    }

    #[inline]
    async fn get_path_vars(&mut self) -> Option<PathVars> {
        self.transient()
            .await
            .current_resource::<PathVars>(ResourceKey::root())
    }

    #[inline]
    async fn take_request(&self) -> Option<PoemRequest> {
        self.transient_mut()
            .await
            .take_resource::<PoemRequest>(ResourceKey::root())
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
#[derive(Reality, Serialize, Deserialize, PartialEq, PartialOrd, Default)]
#[reality(plugin, call = start_engine_proxy, rename = "engine-proxy", group = "loopio.poem")]
pub struct EngineProxy {
    /// Address to host the proxy on,
    ///
    #[reality(derive_fromstr)]
    address: String,
    /// If set, maps this alias to the address of this proxy
    ///
    #[reality(option_of=String)]
    alias: Option<String>,
    ///
    ///
    #[reality(map_of=Decorated<String>)]
    path: BTreeMap<String, Decorated<String>>,
    /// Map of routes to the operations they map to,
    ///
    #[reality(map_of=Decorated<Address>)]
    route: BTreeMap<String, Decorated<Address>>,
}

impl Debug for EngineProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineProxy")
            .field("address", &self.address)
            .field("alias", &self.alias)
            .field("path", &self.path)
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
            path: self.path.clone(),
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
    resource.context_mut().reset();

    resource.context_mut().transient_mut().await.put_resource(
        PoemRequest {
            path: path_vars,
            uri: req.uri().clone(),
            headers: req.headers().clone(),
            body: Some(body),
        },
        ResourceKey::root(),
    );

    if let Some(spawned) = resource.spawn() {
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
                match (context.take_response_parts(), context.take_body()) {
                    (Some(parts), Some(body)) => Ok(poem::Response::from_parts(parts, body)),
                    _ => Ok(poem::Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .finish()),
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

/// Starts the engine proxy,
///
async fn start_engine_proxy(context: &mut ThunkContext) -> anyhow::Result<()> {
    let initialized = Remote.create::<EngineProxy>(context).await;

    // Find hosted resources to route to
    let mut resources = BTreeMap::new();
    for (setting, handler) in initialized.route.iter().filter_map(|(k, v)| {
        if v.value().is_some() {
            Some((k, v))
        } else {
            None
        }
    }) {
        let address = handler.value().expect("should exist just checked");
        if let Some(eh) = context.engine_handle().await {
            match eh.hosted_resource(address.to_string()).await {
                Ok(resource) => {
                    trace!("Adding hosted resource for setting - {setting}");
                    resources.insert(setting, resource);
                }
                Err(err) => {
                    error!("Could not find hosted resource -- {err}");
                }
            }
        } else {
            error!("Did not have engine handle");
        }
    }

    // Create route handler
    let route = initialized
        .path
        .iter()
        .fold(Route::new(), |route, (setting, path)| {
            let ep = |setting| {
                if let Some(resource) = resources.get(&setting) {
                    on_proxy.data(resource.clone())
                } else {
                    // TODO -- Create a "landing page hosted resource"
                    on_proxy.data(HostedResource::default())
                }
            };

            // Setting name
            if let Some(methods) = path
                .property("methods")
                .and_then(|m| CommaSeperatedStrings::from_str(m).ok())
            {
                // Parse the methods from decoration properties
                let methods = methods
                    .into_iter()
                    .filter_map(|m| Method::from_str(&m).ok());
                let route_method = methods.fold(RouteMethod::new(), move |route, m| match m {
                    Method::GET => route.get(ep(setting)),
                    Method::HEAD => route.head(ep(setting)),
                    Method::OPTIONS => route.options(ep(setting)),
                    Method::PUT => route.put(ep(setting)),
                    Method::POST => route.post(ep(setting)),
                    Method::PATCH => route.patch(ep(setting)),
                    Method::DELETE => route.delete(ep(setting)),
                    Method::CONNECT => route.connect(ep(setting)),
                    Method::TRACE => route.trace(ep(setting)),
                    _ => route,
                });

                if let Some(path) = path.value() {
                    route.at(path, route_method)
                } else {
                    route
                }
            } else if let Some(path) = path.value() {
                route.at(path, get(ep(setting)))
            } else {
                route
            }
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

        eprintln!("Adding alias: {:?} -> {:?}", alias, replace_with);
        context
            .register_internal_host_alias(alias?, replace_with?)
            .await;
        context.on_notify_host(scheme.as_str()).await?;

        // TODO: Plugins can "opt-in" to eventing
        // {host}://?event=engine-proxy-started
        // --> .events.as_map().get("engine-proxy-started")
        // let bus = context.virtual_bus::<Event>(scheme.as_str().parse::<Address>()?).await;

        // bus.transmit().await.write_to_virtual(|u| {
        //     u.virtual_mut().events.as_map().get("engine-proxy-started").commit()
        // });

        //
        // TODO -- context.wire_bus("demo://").commit(virtual_engine_proxy.path);
        //      or context.virtual_bus("demo://"). api's -- wait_for, commit, changed,
        //
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

/// Reverse proxy config,
///
#[derive(Reality, Serialize, Deserialize, Clone, PartialEq, Default)]
#[reality(plugin, call = configure_reverse_proxy, rename = "reverse-proxy-config", group = "loopio.poem")]
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
#[reality(plugin, call = start_reverse_proxy, rename = "reverse-proxy", group = "loopio.poem")]
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

    let _bus = tc.virtual_bus(init.address.parse::<Address>()?).await;

    // TODO -- Get the address of the engine_proxies

    // // let mut routes = BTreeMap::new();

    // for host in init.forward.iter() {
    //     let mut transient = tc.transient_mut().await;
    //     let resource = transient
    //         .take_resource::<EngineProxy>(Some(ResourceKey::with_hash(host.as_ref().to_string())));
    //     eprintln!("Processing reverse proxy config for {}", host.as_ref(),);
    //     // if let Some(resource) = resource {
    //     //     for (address, route_method) in resource.routes {
    //     //         eprintln!("Forwarding route {}", address);
    //     //         routes.insert(address, route_method);
    //     //     }
    //     // }
    // }

    // let mut route = Route::new();
    // for (address, _route) in routes {
    //     route = route.at(address, _route);
    // }

    // let listener = TcpListener::bind(&init.address);
    // eprintln!("Listening to {}", init.address);

    // poem::Server::new(listener)
    //     .run_with_graceful_shutdown(route, tc.cancellation.child_token().cancelled(), None)
    //     .await?;

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

    if let (Some(_host), Some(internal_host)) = (
        init.alias.as_ref().scheme_str(),
        tc.internal_host_lookup(init.alias.as_ref()).await,
    ) {
        let client = Arc::new(hyper_ext::local_client());
        let _client = &client;

        let internal_host = Arc::new(internal_host);
        let _internal_host = &internal_host;

        // if let Some(mut engine_proxy) = tc.scan_host_for::<EngineProxy>(host).await {
        //     println!("Configuring reverse proxy for {}", init.alias.as_ref());
        //     let config = || init.clone().decorate(on_forward_request);
        //     // create_routes!(
        //     //     move || { config().data(client.clone()).data(internal_host.clone()) },
        //     //     tc,
        //     //     engine_proxy,
        //     //     [head, get, post, put, patch, delete]
        //     // );

        //     tc.transient_mut().await.put_resource(
        //         engine_proxy,
        //         Some(ResourceKey::with_hash(init.alias.to_string())),
        //     );
        // }
    }
    Ok(())
}
