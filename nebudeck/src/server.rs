use hyper::HeaderMap;
use hyper::StatusCode;
use hyper::Version;
use loopio::engine::Engine;
use loopio::operation::Operation;
use poem::get;
use poem::http::Extensions;
use poem::listener::Acceptor;
use poem::listener::Listener;
use poem::listener::TcpAcceptor;
use poem::listener::TcpListener;
use poem::web::Data;
use poem::Body;
use poem::EndpointExt;
use poem::Response;
use poem::ResponseParts;
use poem::web::LocalAddr;
use reality::ResourceKey;
use reality::StorageTarget;
use tracing::info;

use crate::BackgroundWork;
use crate::ControlBus;
use crate::Controller;

/// Struct containing a listener and routes for hosting a poem server,
///
pub struct Server<A: Acceptor> {
    /// Routes hosted by this server,
    /// 
    pub routes: poem::Route,
    /// Acceptor listening for incoming requests,
    /// 
    acceptor: A,
}

impl<A: Acceptor> Server<A> {
    /// Creates a new server with a tcp listener bounded to a port selected by the OS,
    /// 
    pub async fn new() -> anyhow::Result<Server<TcpAcceptor>> {
        Ok(Server {
            acceptor: TcpListener::bind("localhost::").into_acceptor().await?,
            routes: poem::Route::new(),
        })
    }

    /// Creates a new server w/ listener,
    /// 
    pub async fn new_with<L: Listener>(listener: L) -> anyhow::Result<Server<L::Acceptor>> {
        Ok(Server {
            acceptor: listener.into_acceptor().await?,
            routes: poem::Route::new(),
        })
    }

    /// Returns the addresses for the server,
    ///  
    pub fn addr(&self) -> Vec<LocalAddr> {
        self.acceptor.local_addr()
    }
}

/// Trait for a ControlBus implementation to customize the server at different parts,
///
pub trait ServerApp: ControlBus {
    /// Create routes,
    ///
    fn create_routes(&self, routes: poem::Route) -> poem::Route;
}

impl<C: ServerApp, L: Acceptor + 'static> Controller<C> for Server<L> {
    fn take_control(self, engine: loopio::engine::Engine) -> BackgroundWork {
        let cancellation = engine.cancellation.child_token();
        let handle = engine.handle();
        let app = C::create(engine);

        let routes = app.create_routes(self.routes);

        Some(handle.spawn(async move {
            poem::Server::new_with_acceptor(self.acceptor)
                .run_with_graceful_shutdown(routes, cancellation.cancelled(), None)
                .await?;

            Ok(())
        }))
    }
}

/// Hosts the operations of the engine remotely,
///
pub struct RemoteEngine(Engine);

/// Maps http request into transient storage before executing an engine operation,
///
#[poem::handler]
async fn run_operation(
    request: &poem::Request,
    body: Body,
    operation: Data<&Operation>,
) -> Response {
    let mut op = operation.clone();
    if let Some(op) = op.context_mut() {
        op.reset();
        let transient = op.transient();
        let mut storage = transient.storage.write().await;

        let headers = request.headers().clone();
        storage.put_resource(headers, None);
        let uri = request.uri().clone();
        storage.put_resource(uri, None);
        storage.put_resource(body, None);
        storage.put_resource(
            ResponseParts {
                status: StatusCode::OK,
                version: Version::HTTP_11,
                headers: HeaderMap::new(),
                extensions: Extensions::new(),
            },
            None,
        );
        storage.put_resource(Body::empty(), Some(ResourceKey::with_hash("response")));
        storage.put_resource(request.method().clone(), None);
    }

    if let Ok(op) = op.execute().await {
        let transient = op.transient();
        let mut storage = transient.storage.write().await;

        if let Some(response) = storage.take_resource::<Response>(None) {
            return *response;
        } else if let (Some(parts), Some(body)) = (
            storage.take_resource::<ResponseParts>(None),
            storage.take_resource::<Body>(Some(ResourceKey::with_hash("response"))),
        ) {
            return Response::from_parts(*parts, *body);
        }
    }

    Response::builder().status(StatusCode::BAD_REQUEST).finish()
}

impl ControlBus for RemoteEngine {
    fn create(engine: Engine) -> Self {
        Self(engine)
    }
}

impl ServerApp for RemoteEngine {
    fn create_routes(&self, routes: poem::Route) -> poem::Route {
        let mut route_list = vec![];

        self.0
            .iter_operations()
            .fold(routes, |routes, (address, op)| {
                info!("Setting route {address}");
                let address = address.replace("#", "/_tag/");
                route_list.push(address.to_string());
                routes.at(
                    address,
                    get(run_operation.data(op.clone())).post(run_operation.data(op.clone())),
                )
            })
    }
}
