use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use imgui::Ui;
use imgui_wgpu::RendererConfig;
use imgui_winit_support::WinitPlatform;
use tracing::error;
use winit::window::Window;

use loopio::prelude::*;
use winit_27::event_loop::EventLoopProxy;

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
    /// Handle to the compiled engine,
    ///
    engine: OnceCell<EngineHandle>,
    /// Imgui context,
    ///
    context: OnceCell<imgui::Context>,
    /// Winit platform support,
    ///
    platform: OnceCell<imgui_winit_support::WinitPlatform>,
    /// Wgpu renderer support,
    ///
    renderer: OnceCell<imgui_wgpu::Renderer>,
    /// If Some, enables the demo window,
    ///
    open_demo: Option<bool>,
    /// The last frame time this middleware processed,
    ///
    last_frame: Option<Instant>,
    /// Vector of active ui nodes,
    /// 
    pub ui_nodes: Vec<UiNode>,
    /// Unused,
    ///
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
            ui_nodes: vec![],
            _t: PhantomData,
        }
    }

    /// Enables the demo window,
    ///
    pub fn enable_demo_window(mut self) -> Self {
        self.open_demo = Some(true);
        self
    }

     /// Enables the demo window,
    ///
    pub fn with_ui_node(mut self, ui_node: UiNode) -> Self {
        self.ui_nodes.push(ui_node);
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
        if let (Some(imgui_context), Some(platform)) =
            (self.context.get_mut(), self.platform.get_mut())
        {
            platform.attach_window(
                imgui_context.io_mut(),
                &window,
                imgui_winit_support::HiDpiMode::Default,
            );

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
    fn before_event_loop(&mut self, _: &winit::window::Window, _: EventLoopProxy<T>) {
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

                if let Some(open_demo_window) = self.open_demo.as_mut() {
                    ui.show_demo_window(open_demo_window);
                }

                for uinode in self.ui_nodes.iter_mut() {
                    uinode.show(&ui);
                }

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

#[async_trait]
pub trait ImguiExt {
    async fn add_ui_node(&self, show: impl for<'a, 'b> Fn(&'a mut ThunkContext, &'b Ui) -> bool + Send + Sync + 'static);
}

#[async_trait]
impl ImguiExt for ThunkContext {
    async fn add_ui_node(&self, show: impl for<'a, 'b> Fn(&'a mut ThunkContext, &'b Ui) -> bool + Send + Sync + 'static) {
        let ui_node = UiNode {
            show_ui: Some(Arc::new(show)),
            context: self.clone()
        };

        unsafe { self.node_mut().await.put_resource(ui_node, self.attribute.map(|a| a.transmute())) };
    }
}

pub type ShowUi = Arc<dyn Fn(&mut ThunkContext, &Ui) -> bool + Sync + Send + 'static>;

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
                if let Ok(target) = target {
                    let opened = Arc::new(T::open);
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

/// UI Node contains a rendering function w/ a thunk context,
/// 
#[derive(Clone)]
pub struct UiNode {
    /// Dispatcher for this ui node,
    ///
    pub context: ThunkContext,
    /// Function to show ui,
    ///
    pub show_ui: Option<ShowUi>,
}

impl UiNode {
    /// Shows the ui attached to a node,
    /// 
    pub fn show(&mut self, ui: &Ui) {
        if let Some(show) = self.show_ui.as_ref() {
            show(&mut self.context, ui);
        }
    }
}
