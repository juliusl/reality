//! # Nebudeck
//! 
//! This library is a comprehensive suite of front-end controllers,
//! 
//! ## Controller Types
//! 
//! - Desktop: Applications w/ a GUI that are accessed from a Desktop environment
//! - Terminal: Applications based on terminal utilities
//! 
//! ## Extensions 
//! 
//! **Desktop Extensions**
//! - desktop-wgpu: Adds extensions for working with wgpu for rendering to the desktop window
//! - desktop-imgui: Adds extensions for adding developer UI w/ imgui
//! - desktop-softbuffer: (TODO) Adds extensions for working with softbuffer for rendering to the desktop window
//! 
//! 
mod controller;
pub use controller::Controller;
pub use controller::ControlBus;
pub use controller::BackgroundWork;

pub mod ext;

#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "desktop-imgui")]
pub mod widgets;
#[cfg(feature = "terminal")]
pub mod terminal;