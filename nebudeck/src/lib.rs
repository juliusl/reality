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
pub use controller::BackgroundWork;
pub use controller::ControlBus;
pub use controller::Controller;

mod project;

pub mod ext;

mod nebudeck;
pub use nebudeck::set_nbd_boot_only;
pub use nebudeck::set_nbd_boot_prog;
pub use nebudeck::Nebudeck;

#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "desktop-imgui")]
pub mod widgets;

pub mod terminal;

mod base64 {
    use loopio::prelude::FieldPacket;

    pub fn decode_field_packet(val: impl AsRef<str>) -> anyhow::Result<FieldPacket> {
        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, val.as_ref())?;

        let decoded = bincode::deserialize(&decoded)?;

        Ok(decoded)
    }
}
