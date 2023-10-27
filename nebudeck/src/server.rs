use poem::listener::Listener;
use tracing::info;
use tracing::error;

use crate::{ControlBus, Controller};

/// Struct containing a listener and routes for hosting a poem server,
/// 
pub struct Server<L: Listener> {
    listener: L,
    routes: poem::Route,
}

/// Trait for a ControlBus implementation to customize the server at different parts,
/// 
pub trait ServerApp: ControlBus {
    /// Create routes,
    /// 
    fn create_routes(&self, routes: poem::Route) -> poem::Route;
}

impl<C: ServerApp, L: Listener + 'static> Controller<C> for Server<L> {
    fn take_control(self, engine: loopio::engine::Engine) {
        let cancellation = engine.cancellation.child_token();
        let app = C::create(engine);

        let routes = app.create_routes(self.routes);

        let server = poem::Server::new(self.listener).run_with_graceful_shutdown(
            routes,
            cancellation.cancelled(),
            None,
        );

        match tokio::runtime::Handle::current().block_on(server) {
            Ok(_) => {
                info!("Server is exiting.");
            }
            Err(err) => {
                error!("Server encountered an error: {err}. Server is exiting.")
            }
        }
    }
}
