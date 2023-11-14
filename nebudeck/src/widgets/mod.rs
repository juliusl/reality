mod imgui_frame_editor;

pub mod prelude {
    #[cfg(feature = "desktop-imgui")]
    pub use super::imgui_frame_editor::FrameEditor;
}

pub use prelude::*;