mod button;
mod frame_editor;
mod input;
mod servers;

pub mod prelude {
    use anyhow::anyhow;
    use loopio::engine::EngineHandle;
    use loopio::prelude::Attribute;
    use loopio::prelude::CacheExt;
    use loopio::prelude::Dispatcher;
    use loopio::prelude::FieldPacket;
    use loopio::prelude::FrameUpdates;
    use loopio::prelude::KvpExt;
    use loopio::prelude::ResourceKey;
    use loopio::prelude::Shared;
    use loopio::prelude::ThunkContext;
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::sync::Mutex;
    use std::sync::OnceLock;
    use std::sync::RwLock;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::FrameEditor;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::EditorWidgetTable;

    #[cfg(feature = "desktop-imgui")]
    pub use super::frame_editor::FieldWidget;

    // pub use super::button::Button;
    pub use super::input::Input;

    pub use super::servers::Servers;

    pub struct SectionBody {
        inner: Vec<fn(&UiFormatter<'_>)>,
    }

    impl UiDisplayMut for SectionBody {
        fn fmt(&mut self, ui: &UiFormatter<'_>) -> anyhow::Result<()> {
            for i in self.inner.iter() {
                i(ui);
            }
            Ok(())
        }
    }

    /// Ui formatter context,
    ///
    pub struct UiFormatter<'frame> {
        /// Resource key set on the current formatter,
        ///
        pub(crate) rk: ResourceKey<Attribute>,
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
        /// Current frame updates,
        ///
        pub frame_updates: RefCell<FrameUpdates>,
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
                let (_, mut s) = tc
                    .get_mut()
                    .unwrap()
                    .maybe_store_kv::<Vec<fn(&UiFormatter<'_>)>>(name, vec![]);
                s.push(show);
            }
        }

        /// Takes any pending section show fn and passes it to a function that can format the section,
        ///
        pub fn show_section(&self, name: &str, show: fn(&str, &UiFormatter<'_>, SectionBody)) {
            if let Ok(mut tc) = self.context_mut() {
                if let Some((_, s)) = tc
                    .get_mut()
                    .unwrap()
                    .take_kv::<Vec<fn(&UiFormatter<'_>)>>(name)
                {
                    drop(tc);
                    show(name, self, SectionBody { inner: s });
                }
            }
        }

        /// Pushes a pending change to a label,
        ///
        /// **Note**: If a previous packet is present, it will be replaced by this packet.
        ///
        pub fn push_pending_change(&self, label: &str, packet: FieldPacket) {
            if let Ok(mut tc) = self.context_mut() {
                let tc = tc.get_mut().unwrap();
                // Insert the label to pending changes,
                {
                    let mut pending = tc.maybe_write_cache(BTreeSet::new());
                    pending.insert(label.to_string());
                }

                // Insert the new field packet,
                {
                    tc.store_kv(label, packet);
                }
            }
        }

        /// Apply func to each pending change,
        ///
        pub fn for_each_pending_change(&self, mut func: impl FnMut(&str, &FieldPacket)) -> usize {
            if let Ok(tc) = self.context_mut() {
                let tc = tc.get().unwrap();
                if let Some(pending) = tc.cached_ref::<BTreeSet<String>>() {
                    for p in pending.iter() {
                        if let Some((_, fp)) = tc.fetch_kv::<FieldPacket>(p.as_str()) {
                            func(p, &fp);
                        }
                    }

                    return pending.len();
                }
            }
            0
        }

        /// Shows the call button if applicable,
        ///
        pub fn show_call_button(&self) {
            // if let Ok(deco) = self.decorations.read() {
            //     self.imgui.text(format!("{:#?}", deco));

            //     if let Some(address) = deco
            //         .get()
            //         .and_then(|d| d.comment_properties.as_ref())
            //         .and_then(|d| d.get("address"))
            //     {
            //         if let Some(bg) = self.eh.lock().unwrap().background() {
            //             if let Ok(mut call) = bg.call(address) {
            //                 match call.status() {
            //                     loopio::background_work::CallStatus::Enabled => {
            //                         if self.imgui.button("Run") {
            //                             call.spawn_with_updates(
            //                                 self.frame_updates.replace(FrameUpdates::default()),
            //                             );
            //                         }
            //                     }
            //                     loopio::background_work::CallStatus::Disabled => {}
            //                     loopio::background_work::CallStatus::Running => {
            //                         self.imgui.text("Running");

            //                         self.imgui.same_line();
            //                         if self.imgui.button("Cancel") {
            //                             call.cancel();
            //                         }
            //                     }
            //                     loopio::background_work::CallStatus::Pending => {
            //                         let _ = call.into_foreground().unwrap();
            //                         eprintln!("Background work finished");
            //                     }
            //                 }
            //             }
            //         }
            //     }
            // }
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
                    if arg.is_required_set() {}

                    match arg.get_action() {
                        clap::ArgAction::Set => {}
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
