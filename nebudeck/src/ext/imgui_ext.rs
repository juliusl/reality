use std::cell::OnceCell;
use std::sync::Arc;
use std::time::Instant;
use imgui::Ui;
use imgui_wgpu::RendererConfig;
use imgui_winit_support::WinitPlatform;
use loopio::engine::EnginePacket;
use tokio::sync::RwLock;
use tracing::error;
use loopio::prelude::*;
use winit::event_loop::EventLoopProxy;
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
pub struct ImguiMiddleware {
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
    ui_nodes: Vec<UiNode>,
    /// Vector of active ui nodes,
    ///
    ui_type_nodes: Vec<UiTypeNode>,
    /// Vector of aux ui nodes,
    ///
    __aux_ui: Vec<AuxUiNode>,
    /// Update attempted to start,
    ///
    __update_start: Option<Instant>,
    /// When this was last updated,
    ///
    __last_updated: Option<Instant>,
}

impl ImguiMiddleware {
    pub const fn new() -> Self {
        Self {
            engine: OnceCell::new(),
            context: OnceCell::new(),
            platform: OnceCell::new(),
            renderer: OnceCell::new(),
            open_demo: None,
            last_frame: None,
            ui_nodes: vec![],
            ui_type_nodes: vec![],
            __update_start: None,
            __last_updated: None,
            __aux_ui: vec![],
        }
    }

    /// Enables the imgui demo window,
    ///
    pub fn enable_imgui_demo_window(mut self) -> Self {
        self.open_demo = Some(true);
        self
    }

    /// Enables the aux widget demo window,
    ///
    pub fn enable_aux_demo_window(self) -> Self {
        self.with_aux_node(|handle, ui| {
            if let Some(bg) = handle.background() {
                const ADDRESS_INPUT: &'static str = "Address_Input";

                bg.tc.maybe_store_kv(ADDRESS_INPUT, String::new());

                ui.window("Aux-demo Window")
                    .size([800.0, 600.0], imgui::Condition::Once)
                    .build(|| {
                        let mut __address = None;
                        if let Some((_, mut address)) = bg.tc.fetch_mut_kv::<String>(ADDRESS_INPUT) {
                            if ui.input_text("Address", &mut address).build() {}
                            __address = Some(address.to_string());
                        }

                        if let Some(address) = __address.take() {
                            if let Ok(mut bg) = bg.call(address.as_str()) {
                                match bg.status() {
                                    loopio::background_work::CallStatus::Enabled => {
                                        if ui.button("Start") {
                                            bg.spawn();
                                        }
                                    }
                                    loopio::background_work::CallStatus::Disabled => {
                                        ui.disabled(true, || {
                                            if ui.button("Start") {
                                            }
                                        })
                                    },
                                    loopio::background_work::CallStatus::Running => {
                                        ui.text("Running");
                                    },
                                    loopio::background_work::CallStatus::Pending => {
                                        bg.into_foreground().unwrap();
                                    },
                                }
                            }

                            // if let Ok(_bg) = bg.listen(&address) {
                            // }
                        }
                    });
            }
            true
        })
    }

    /// Enables the demo window,
    ///
    pub fn with_ui_node(mut self, ui_node: UiNode) -> Self {
        self.ui_nodes.push(ui_node);
        self
    }

    /// Adds an auxilary node,
    ///
    pub fn with_aux_node(
        mut self,
        aux_ui: impl FnMut(&mut EngineHandle, &Ui) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.__aux_ui.push(AuxUiNode {
            engine_handle: None,
            show_ui: Arc::new(RwLock::new(aux_ui)),
        });
        self
    }

    /// Update any ui nodes,
    ///
    pub fn update(&mut self) {
        if let Some(eh) = self.engine.get_mut() {
            if let Some(bg) = eh.background() {
                if let Ok(mut scanner) = bg.call("engine://scan-ui-nodes") {
                    match scanner.status() {
                        loopio::background_work::CallStatus::Pending => {
                            if let Ok(_fg) = scanner.into_foreground() {
                                let s = _fg.transient.storage.clone();
                                if let Ok(mut s) = s.try_write() {
                                    if let Some(nodes) = s.take_resource::<Vec<UiNode>>(ResourceKey::root()) {
                                        self.ui_nodes.extend(*nodes);
                                    }
                                };
                            }
                        },
                        _ => {
                            // let listen = scanner.listen();
                            // match listen.status() {
                            //     loopio::background_work::CallStatus::Enabled => todo!(),
                            //     loopio::background_work::CallStatus::Disabled => todo!(),
                            //     loopio::background_work::CallStatus::Running => todo!(),
                            //     loopio::background_work::CallStatus::Pending => todo!(),
                            // }
                        }
                    }
                }
            }
        }
    }
}

impl Default for ImguiMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderPipelineMiddleware for ImguiMiddleware {
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
        _: &mut wgpu::util::StagingBelt,
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

impl DesktopApp for ImguiMiddleware {
    fn before_event_loop(&mut self, _: &winit::window::Window, _: EventLoopProxy<EnginePacket>) {
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
        event: &winit::event::Event<EnginePacket>,
        context: &crate::desktop::DesktopContext,
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

        // TOOD: This could be placed on a better handler to reduce overhead
        self.update();
    }

    fn on_user_event(&mut self, _user: &EnginePacket, _context: &crate::desktop::DesktopContext) {
        self.update();
    }

    fn on_window_redraw(
        &mut self,
        _: winit::window::WindowId,
        context: &crate::desktop::DesktopContext,
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
                    if !uinode.show(&ui) {
                        // TODO -- "Close the node"
                    }
                }

                for auxnode in self.__aux_ui.iter_mut() {
                    if auxnode.engine_handle.is_none() {
                        auxnode.engine_handle = Some(
                            self.engine
                                .get()
                                .cloned()
                                .expect("should have an engine handle by this point"),
                        );
                    }

                    if !auxnode.show(&ui) {
                        // TODO -- "Close the node"
                    }
                }

                for ui_type_node in self.ui_type_nodes.iter_mut() {
                    if !ui_type_node.show(&ui) {
                        // TODO -- "Close the node"
                    }
                }

                platform.prepare_render(&ui, context.window);
            }
        }
    }
}

impl ControlBus for ImguiMiddleware {
    fn bind(&mut self, engine: EngineHandle) {
        // {
        //     // let stream = engine.scan_take_nodes::<UiNode>();
        //     pin_mut!(stream);

        //     let mut stream = futures::executor::block_on_stream(stream);

        //     while let Some(node) = stream.next() {
        //         self.ui_nodes.push(node);
        //     }
        // }

        self.engine.set(engine).expect("should only be called once");
    }
}

impl ImguiMiddleware {
    fn __middleware_tools(
        &mut self,
        _: winit::window::WindowId,
        _: &crate::desktop::DesktopContext,
        _ui: &Ui,
    ) {
    }
}

#[async_trait]
pub trait ImguiExt {
    async fn add_ui_node(
        &self,
        show: impl for<'a, 'b> Fn(&'a mut ThunkContext, &'b Ui) -> bool + Send + Sync + 'static,
    );

    async fn add_ui_type_node<G: Default + Send + Sync + 'static>(
        &self,
        show: impl for<'a, 'b> Fn(&'a mut Dispatcher<Shared, Attribute>, &'b Ui) -> bool
            + Send
            + Sync
            + 'static,
    );
}

#[async_trait]
impl ImguiExt for ThunkContext {
    async fn add_ui_node(
        &self,
        show: impl for<'a, 'b> Fn(&'a mut ThunkContext, &'b Ui) -> bool + Send + Sync + 'static,
    ) {
        let ui_node = UiNode {
            show_ui: Some(Arc::new(show)),
            context: self.clone(),
        };

        unsafe {
            self.node_mut()
                .await
                .put_resource(ui_node, self.attribute.transmute());
        };
    }

    async fn add_ui_type_node<G: Default + Send + Sync + 'static>(
        &self,
        show: impl for<'a, 'b> Fn(&'a mut Dispatcher<Shared, Attribute>, &'b Ui) -> bool
            + Send
            + Sync
            + 'static,
    ) {
        let ui_node = UiTypeNode {
            show_ui: Some(Arc::new(show)),
            dispatcher: self.dispatcher::<G>().await.transmute(),
        };

        unsafe {
            self.node_mut()
                .await
                .put_resource(ui_node, self.attribute.transmute())
        };
    }
}

/// Type-alias for a dispatcher based UI signature,
///
pub type ShowTypeUi =
    Arc<dyn Fn(&mut Dispatcher<Shared, Attribute>, &Ui) -> bool + Sync + Send + 'static>;

/// Type-alias for a plugin-based UI function signature,
///
pub type ShowUi = Arc<dyn Fn(&mut ThunkContext, &Ui) -> bool + Sync + Send + 'static>;

/// Type-alias for an engine handle based UI function signature,
///
pub type AuxUi = Arc<RwLock<dyn FnMut(&mut EngineHandle, &Ui) -> bool + Sync + Send + 'static>>;

/// UI Node contains a rendering function w/ a thunk context,
///
#[derive(Clone)]
pub struct UiTypeNode {
    /// Dispatcher for this ui node,
    ///
    pub dispatcher: Dispatcher<Shared, Attribute>,
    /// Function to show ui,
    ///
    pub show_ui: Option<ShowTypeUi>,
}

impl UiTypeNode {
    /// Shows the ui,
    ///
    pub fn show(&mut self, ui: &Ui) -> bool {
        if let Some(show) = self.show_ui.as_ref() {
            show(&mut self.dispatcher, ui)
        } else {
            false
        }
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

/// Auxilary UI node, containing a rendering function w/ engine handle,
///
pub struct AuxUiNode {
    /// Engine handle,
    ///
    pub engine_handle: Option<EngineHandle>,
    /// Function to show ui,
    ///
    pub show_ui: AuxUi,
}

impl UiNode {
    /// Shows the ui attached to a node,
    ///
    pub fn show(&mut self, ui: &Ui) -> bool {
        if let Some(show) = self.show_ui.as_ref() {
            show(&mut self.context, ui)
        } else {
            false
        }
    }
}

impl AuxUiNode {
    /// Shows the ui attached to a node,
    ///
    pub fn show(&mut self, ui: &Ui) -> bool {
        if let (Some(handle), Ok(mut show)) =
            (self.engine_handle.as_mut(), self.show_ui.try_write())
        {
            show(handle, ui)
        } else {
            false
        }
    }

    /// Show the UI w/ a different engine handle,
    ///
    /// **Note** When created an aux ui node receives it's own engine handle. This allows
    /// passing a handle directly, such as the middleware's handle.
    ///
    pub fn show_with(&mut self, engine_handle: &mut EngineHandle, ui: &Ui) -> bool {
        if let Ok(mut show) = self.show_ui.try_write() {
            show(engine_handle, ui)
        } else {
            false
        }
    }
}
