use imgui::Ui;
use loopio::prelude::*;
use loopio::address::Address;
use loopio::prelude::Reality;
use loopio::background_work::BackgroundWorkEngineHandle;

use crate::widgets::UiDisplayMut;

use super::UiFormatter;

/// Struct for a button,
/// 
#[derive(Reality, Default, Clone)]
#[reality(group = "ui")]
pub struct Button {
    /// Address this button is connected to,
    /// 
    #[reality(derive_fromstr)]
    address: Address,
    /// Name of the button,
    /// 
    name: String,
    /// Background engine handle,
    ///
    #[reality(ignore, not_wire)]
    bg: Option<BackgroundWorkEngineHandle>,
}

impl UiDisplayMut for Button {
    fn fmt(&mut self, ui: &mut UiFormatter<'_>) -> anyhow::Result<()> {
        let ui = &ui.imgui;

        if let Some(bg) = self.bg.as_mut() {
            match bg.call(self.address.to_string().as_str()) {
                Ok(mut bg) => match bg.status() {
                    loopio::background_work::CallStatus::Enabled => {
                        if ui.button("Start") {
                            bg.spawn();
                        }
                    }
                    loopio::background_work::CallStatus::Disabled => {
                        ui.disabled(true, || if ui.button("Start") {})
                    }
                    loopio::background_work::CallStatus::Running => {
                        ui.text("Running");
                    }
                    loopio::background_work::CallStatus::Pending => {
                        bg.clone().into_foreground().unwrap();
                    }
                },
                Err(err) => {
                    ui.text(format!("Error: {err}"));
                },
            }
        } else {
            ui.text("Error: background engine handle is not set");
        }

        Ok(())
    }
}