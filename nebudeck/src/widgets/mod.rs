mod button;
mod frame_editor;
mod input;

pub mod prelude {
    use loopio::engine::EngineHandle;
    use loopio::prelude::Attribute;
    use loopio::prelude::Dispatcher;
    use loopio::prelude::Shared;
    use loopio::prelude::ThunkContext;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::FrameEditor;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::EditorWidgetTable;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::FieldWidget;

    pub use super::button::Button;
    pub use super::input::Input;

    /// Ui formatter context,
    ///
    pub struct UiFormatter<'frame> {
        /// Handle to the ui builder,
        ///
        pub imgui: &'frame mut imgui::Ui,
        /// Engine handle,
        ///
        pub eh: EngineHandle,
        /// Currently available subcommand,
        /// 
        #[cfg(feature = "terminal")]
        pub subcommand: Option<clap::Command>,
        /// Currently available thunk context,
        ///
        pub tc: Option<ThunkContext>,
        /// Currently available dispatcher,
        ///
        pub disp: Option<Dispatcher<Shared, Attribute>>,
    }

    /// Trait for formatting ui controls for a type w/ a mutable reference,
    ///
    pub trait UiDisplayMut {
        /// Formats the ui w/ a mutable reference to the implementing type,
        ///
        fn fmt(&mut self, ui: &mut UiFormatter<'_>) -> anyhow::Result<()>;
    }

    #[cfg(feature = "desktop-imgui")]
    impl UiFormatter<'_> {
        /// Show w/ thunk context,
        /// 
        pub fn show_with_tc(&mut self, show: impl FnOnce(&mut ThunkContext, &imgui::Ui)) {
            if let Some(tc) = self.tc.as_mut() {
                show(tc, self.imgui)
            }
        }

        /// Show w/ engine handle,
        /// 
        pub fn show_with_eh(&mut self, show: impl FnOnce(EngineHandle, &imgui::Ui)) {
            show(self.eh.clone(), self.imgui)
        }

        /// Show w/ all resources,
        /// 
        pub fn show_with_all(&mut self, show: impl FnOnce(EngineHandle, &mut ThunkContext, &imgui::Ui)) {
            if let Some(tc) = self.tc.as_mut() {
                show(self.eh.clone(), tc, self.imgui)
            }
        }

        /// Display a menu corresponding the current subcommand config,
        /// 
        pub fn show_subcommand(&mut self) {
            if let Some(subcommand) = self.subcommand.as_ref() {
                for arg in subcommand.get_arguments() {
                    //
                    if arg.is_required_set() {

                    }

                    match arg.get_action() {
                        clap::ArgAction::Set => {
                        
                        },
                        clap::ArgAction::Append => todo!(),
                        clap::ArgAction::SetTrue => todo!(),
                        clap::ArgAction::SetFalse => todo!(),
                        clap::ArgAction::Count => todo!(),
                        clap::ArgAction::Help => todo!(),
                        clap::ArgAction::HelpShort => todo!(),
                        clap::ArgAction::HelpLong => todo!(),
                        clap::ArgAction::Version => todo!(),
                        _ => todo!(),
                    }
                }
            }
        }
    }
}

pub use prelude::*;
