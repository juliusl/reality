mod imgui_frame_editor;

pub mod prelude {
    #[cfg(feature = "desktop-imgui")]
    pub use super::imgui_frame_editor::FrameEditor;

    #[cfg(feature = "desktop-imgui")]
    pub use super::imgui_frame_editor::EditorWidgetTable;

    #[cfg(feature = "desktop-imgui")]
    pub use super::imgui_frame_editor::FieldWidget;
}

pub use prelude::*;