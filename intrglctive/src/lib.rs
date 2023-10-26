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
pub mod project_loop;

#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "terminal")]
pub mod terminal;