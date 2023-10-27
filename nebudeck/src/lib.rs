//! # Nebudeck
//! 
//! This library is a comprehensive suite of front-end implementations.
//! 
//! ## Interaction Loop Types
//! 
//! - Desktop: Applications w/ a GUI that are accessed from a Desktop environment
//! - Terminal: Applications based on terminal utilities
//! - Server: Application based on an HTTP API Server
//! 
mod controller;
pub use controller::Controller;
pub use controller::ControlBus;
pub use controller::BackgroundWork;

pub mod deck;

#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "terminal")]
pub mod terminal;
#[cfg(feature = "server")]
pub mod server;