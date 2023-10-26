//! # Intrglctive
//! 
//! This library is a collection of interaction loops w/ integrations into reality created types.
//! 
//! ## Interaction Loop Types
//! 
//! - Desktop: Applications w/ a GUI that are accessed from a Desktop environment
//! - Terminal: Applications based on terminal utilities
//! 
//! 
mod project_loop;
pub use project_loop::ProjectLoop;
pub use project_loop::InteractionLoop;
pub use project_loop::AppType;

pub mod deck;

#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "terminal")]
pub mod terminal;