#[cfg(feature = "desktop-wgpu")]
mod wgpu_ext;
#[cfg(feature = "desktop-wgpu")]
pub use wgpu_ext::WgpuResourceManagementExt;