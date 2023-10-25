use std::{cell::OnceCell, marker::PhantomData};

use reality::{BlockObject, Dispatcher, ExtensionController, Shared, StorageTarget};
use tokio::sync::Mutex;

/// Trait to allow a type to participate in a "show" loop,
///
pub trait Show<Ui>
where
    Ui: Send + Sync + 'static,
{
    /// Return true to enable show to be called,
    ///
    fn can_show(&self, target: &impl StorageTarget) -> bool;

    /// Called when can_show returns true,
    ///
    fn show<S: StorageTarget + Send + Sync + 'static>(&self, render: Dispatcher<S, Ui>);
}

static IMGUI: tokio::sync::Mutex<OnceCell<imgui::Ui>> = Mutex::const_new(OnceCell::new());

pub async fn enable_imgui(ui: imgui::Ui) -> anyhow::Result<()> {
    let imgui = IMGUI.lock().await;

    let _ = imgui.set(ui);

    Ok(())
}

pub async fn queue_frame<T>(render: impl FnOnce(&imgui::Ui) -> T + 'static) -> anyhow::Result<T> {
    let imgui = IMGUI.lock().await;

    if let Some(ui) = imgui.get() {
        Ok(render(ui))
    } else {
        Err(anyhow::anyhow!("not enabled"))
    }
}

#[derive(Default)]
pub struct TextBox<T> {
    #[cfg(feature = "desktop")]
    imgui: Option<fn(&T, &imgui::Ui) -> anyhow::Result<T>>,
    _i: PhantomData<()>,
}

impl<T: BlockObject<Shared> + Default> ExtensionController<T> for TextBox<T> {
    fn ident() -> &'static str {
        "ui/textbox"
    }

    fn setup(
        resource_key: Option<&reality::ResourceKey<reality::Attribute>>,
    ) -> reality::Extension<Self, T> {
        Self::default_setup(resource_key).user_task(|textbox, _, obj| {
            let textbox = textbox.clone();
            Box::pin(async move {
                if let Some(render) = textbox.imgui.as_ref().cloned() {
                    let result = queue_frame(move |ui| render(obj.as_ref().unwrap(), ui)).await;
                    result?
                } else {
                    obj
                }
            })
        })
    }
}
