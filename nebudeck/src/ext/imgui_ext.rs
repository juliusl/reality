use std::cell::OnceCell;
use std::sync::OnceLock;
use std::time::Instant;

use imgui::Ui;
use imgui_wgpu::RendererConfig;
use imgui_winit_support::WinitPlatform;
use loopio::prelude::*;
use loopio::prelude::{Plugin, SetupTransform};
use tracing::error;
use winit::window::Window;

use crate::desktop::DesktopApp;
use crate::ControlBus;

use super::wgpu_ext::RenderPipelineMiddleware;

pub mod winit {
    #[cfg(feature = "desktop-vnext")]
    pub use winit_29::*;

    #[cfg(feature = "desktop-imgui")]
    pub use winit_27::*;
}

pub mod wgpu {
    #[cfg(feature = "desktop-imgui")]
    pub use wgpu_17::*;
}

/// Wgpu system middleware that enables imgui plugins,
/// 
pub struct ImguiMiddleware<T> {
    engine: OnceCell<EngineHandle>,
    context: OnceCell<imgui::Context>,
    platform: OnceCell<imgui_winit_support::WinitPlatform>,
    renderer: OnceCell<imgui_wgpu::Renderer>,
    open_demo: Option<bool>,
    last_frame: Option<Instant>,
    _t: PhantomData<T>,
}

impl<T: 'static> ImguiMiddleware<T> {
    pub const fn new() -> Self {
        Self {
            engine: OnceCell::new(),
            context: OnceCell::new(),
            platform: OnceCell::new(),
            renderer: OnceCell::new(),
            open_demo: None,
            last_frame: None,
            _t: PhantomData,
        }
    }

    /// Enables the demo window,
    /// 
    pub fn enable_demo_window(mut self) -> Self {
        self.open_demo = Some(true);
        self
    }
}

impl<T: 'static> Default for ImguiMiddleware<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> RenderPipelineMiddleware<T> for ImguiMiddleware<T> {
    fn on_hardware(&mut self, hardware: &super::wgpu_ext::HardwareContext, window: &Window) {
        if let Some(imgui_context) = self.context.get_mut() {

            if let Some(platform) = self.platform.get_mut() {
                platform.attach_window(
                    imgui_context.io_mut(),
                    &window,
                    imgui_winit_support::HiDpiMode::Default,
                );
            }

            imgui_context.set_ini_filename(Some("imgui.conf".into()));

            if let Err(_) = self.renderer.set(imgui_wgpu::Renderer::new(
                imgui_context,
                &hardware.device,
                &hardware.queue,
                RendererConfig {
                    texture_format: hardware.surface_config.format,
                    depth_format: None,
                    ..Default::default()
                },
            )) {
                unreachable!("should only be called once");
            }
        }
    }

    fn on_load_pass<'a: 'b, 'b>(
        &'a mut self,
        rpass: &mut wgpu::RenderPass<'b>,
        _: &wgpu::TextureView,
        hardware: &super::wgpu_ext::HardwareContext,
    ) {
        if let (Some(context), Some(renderer)) = (self.context.get_mut(), self.renderer.get_mut()) {
            if let Err(err) =
                renderer.render(context.render(), &hardware.queue, &hardware.device, rpass)
            {
                error!("Could not render imgui {err}");
            }
        }
    }
}

impl<T: 'static> DesktopApp<T> for ImguiMiddleware<T> {
    fn before_event_loop(&mut self, _: &winit::window::Window) {
        self.context
            .set(imgui::Context::create())
            .expect("should only be called once");

        if let Some(mut imgui_context) = self.context.get_mut() {
            self.platform
                .set(WinitPlatform::init(&mut imgui_context))
                .expect("should only be called once");
        }
    }

    fn before_event(
        &mut self,
        event: &winit::event::Event<T>,
        context: &crate::desktop::DesktopContext<T>,
    ) {
        if let Some(imgui_context) = self.context.get_mut() {
            if let Some(platform) = self.platform.get_mut() {
                platform.handle_event(imgui_context.io_mut(), context.window, event);
            }

            let now = Instant::now();
            if let Some(f) = self.last_frame {
                imgui_context.io_mut().update_delta_time(now - f);
            }

            self.last_frame = Some(now);
        }
    }

    fn on_window_redraw(
        &mut self,
        _: winit::window::WindowId,
        context: &crate::desktop::DesktopContext<T>,
    ) {
        if let (Some(im_context), Some(platform)) =
            (self.context.get_mut(), self.platform.get_mut())
        {
            let io = im_context.io_mut();
            if let Ok(_) = platform.prepare_frame(io, context.window) {
                let ui = im_context.new_frame();

                if let Some(open_demo_window) =  self.open_demo.as_mut() {
                    ui.show_demo_window(open_demo_window);
                }

                // TODO: Scan for user renderers,

                platform.prepare_render(&ui, context.window);
            }
        }
    }
}

impl<T: 'static> ControlBus for ImguiMiddleware<T> {
    fn bind(&mut self, engine: EngineHandle) {
        self.engine.set(engine).expect("should only be called once");
    }
}

pub trait ImguiExt {}

pub type ShowUi = Box<dyn Fn(&mut ThunkContext, &Ui) -> bool + Sync + Send + 'static>;

static UI_HOST: std::sync::OnceLock<(
    tokio::sync::mpsc::Sender<ShowUi>,
    tokio::sync::mpsc::Receiver<ShowUi>,
)> = OnceLock::new();

pub struct ImguiSystem {
    ui_dispatcher: tokio::sync::mpsc::Sender<ShowUi>,
}

impl Default for ImguiSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ImguiSystem {
    pub fn new() -> Self {
        let ui = UI_HOST
            .get_or_init(|| tokio::sync::mpsc::channel(1000))
            .0
            .clone();
        Self { ui_dispatcher: ui }
    }
}

pub trait ImguiTarget
where
    Self: Plugin + Clone + Default,
{
    fn open(context: &mut ThunkContext, ui: &imgui::Ui) -> bool;
}

impl<T> SetupTransform<T> for ImguiSystem
where
    T: Plugin + ImguiTarget,
{
    fn ident() -> &'static str {
        "imgui"
    }

    fn setup_transform(resource_key: Option<&ResourceKey<Attribute>>) -> Transform<Self, T> {
        Self::default_setup(resource_key).before_task(|c, imgui, target| {
            Box::pin(async {
                if let Ok(mut target) = target {
                    let opened = Box::new(T::open);
                    imgui.ui_dispatcher.send(opened).await?;
                    // target.on_show(imgui.ui, None);
                    // Ok((imgui, target))
                    todo!()
                } else {
                }

                Ok((imgui, target))
            })
        })
    }
}