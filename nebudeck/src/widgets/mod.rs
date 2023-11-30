mod button;
mod frame_editor;
mod input;

pub mod prelude {
    use std::sync::Mutex;
    use std::sync::OnceLock;
    use anyhow::anyhow;
    use loopio::engine::EngineHandle;
    use loopio::prelude::Attribute;
    use loopio::prelude::Dispatcher;
    use loopio::prelude::KvpExt;
    use loopio::prelude::Shared;
    use loopio::prelude::ThunkContext;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::FrameEditor;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::EditorWidgetTable;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::FieldWidget;

    // pub use super::button::Button;
    pub use super::input::Input;

    /// Ui formatter context,
    ///
    pub struct UiFormatter<'frame> {
        /// Handle to the ui builder,
        ///
        pub imgui: &'frame mut imgui::Ui,
        /// Engine handle,
        ///
        pub eh: Mutex<EngineHandle>,
        /// Currently available subcommand,
        /// 
        #[cfg(feature = "terminal")]
        pub subcommand: Option<clap::Command>,
        /// Currently available thunk context,
        ///
        pub tc: Mutex<OnceLock<ThunkContext>>,
        /// Currently available dispatcher,
        ///
        pub disp: Option<Dispatcher<Shared, Attribute>>,
    }

    /// Trait for formatting ui controls for a type w/ a mutable reference,
    /// 
    pub trait UiDisplayMut {
        /// Formats the ui w/ a mutable reference to the implementing type,
        ///
        /// **Implementations** **MUST** return OK(()) to indicate that the receiver was mutated, and
        /// in all other cases return an error.
        /// 
        fn fmt(&mut self, ui: &UiFormatter<'_>) -> anyhow::Result<()>;
    }

    #[cfg(feature = "desktop-imgui")]
    impl UiFormatter<'_> {
        /// Pushes a show fn to a section by name,
        /// 
        pub fn push_section(&self, name: &str, show: fn(&UiFormatter<'_>)) {
            if let Ok(mut tc) = self.context_mut() {
                let (_, mut s) = tc.get_mut().unwrap().maybe_store_kv::<Vec<fn(&UiFormatter<'_>)>>(name, vec![]);
                s.push(show);
            }
        }

        /// Takes any pending section show fn and passes it to a function that can format the section,
        /// 
        pub fn show_section(&self, name: &str, show: fn(&UiFormatter<'_>, Vec<fn(&UiFormatter<'_>)>)) {
            if let Ok(mut tc) = self.context_mut() {
                if let Some((_, s)) = tc.get_mut().unwrap().take_kv::<Vec<fn(&UiFormatter<'_>)>>(name) {
                    drop(tc);
                    show(self, s);
                }
            }
        }

        /// Gets a mutable reference to the underlying thunk context,
        /// 
        pub fn context_mut(&self) -> anyhow::Result<std::sync::MutexGuard<OnceLock<ThunkContext>>> {
            self.tc.lock().map_err(|e| anyhow!("{e}"))
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
