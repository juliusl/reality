use std::cell::OnceCell;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use imgui::Ui;
use imgui_wgpu::RendererConfig;
use imgui_winit_support::WinitPlatform;

use loopio::engine::EnginePacket;
use loopio::engine::Published;
use loopio::prelude::*;
use tokio::sync::RwLock;
use tracing::error;

use winit::event_loop::EventLoopProxy;
use winit::window::Window;

use super::wgpu_ext::RenderPipelineMiddleware;
use crate::desktop::DesktopApp;
use crate::widgets::{UiDisplayMut, UiFormatter};
use crate::ControlBus;

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
    user_tool_nodes: Vec<UiNode>,
    /// Vector of active ui nodes,
    ///
    _ui_type_nodes: Vec<UiTypeNode>,
    /// Vector of aux ui nodes,
    ///
    tool_nodes: Vec<(String, ToolUiNode)>,
    /// Update attempted to start,
    ///
    __update_start: Option<Instant>,
    /// When this was last updated,
    ///
    __last_updated: Option<Instant>,
    /// Published paths,
    ///
    pub published: Option<Published>,
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
            user_tool_nodes: vec![],
            _ui_type_nodes: vec![],
            __update_start: None,
            __last_updated: None,
            tool_nodes: vec![],
            published: None,
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
        self.with_aux_node("nbd_demo_window", |handle, imgui, can_show, ui| {
            if let Some(bg) = handle.background() {
                const ADDRESS_INPUT: &'static str = "Address_Input";

                bg.tc.maybe_store_kv(ADDRESS_INPUT, String::new());

                ui.window("Aux-demo Window")
                    .opened(can_show)
                    .size([800.0, 600.0], imgui::Condition::Once)
                    .build(|| {
                        let mut __address = None;

                        if let Some((_, mut address)) = bg.tc.fetch_mut_kv::<String>(ADDRESS_INPUT)
                        {
                            if ui.input_text("Address", &mut address).build() {}
                            __address = Some(address.to_string());
                            if let Some(published) = imgui.published.as_ref() {
                                for a in published.resources.iter().filter_map(|a| a.value()) {
                                    if ui.button(format!("set##{}", a)) {
                                        *address = a.to_string();
                                    }
                                    ui.same_line();
                                    ui.text(a.to_string());
                                }
                            }
                        }

                        if let Some(address) = __address.take() {
                            if let Ok(mut _bg) = bg.call(address.as_str()) {
                                match _bg.status() {
                                    loopio::background_work::CallStatus::Enabled => {
                                        if ui.button("Start") {
                                            let status = _bg.spawn();
                                            eprintln!("Started {:?}", status);
                                        }
                                    }
                                    loopio::background_work::CallStatus::Disabled => {
                                        ui.disabled(true, || if ui.button("Start") {})
                                    }
                                    loopio::background_work::CallStatus::Running => {
                                        ui.text("Running");
                                    }
                                    loopio::background_work::CallStatus::Pending => {
                                        ui.text("Pending");
                                        let __tc = _bg.into_foreground().unwrap();
                                        eprintln!(
                                            "Background work finished {}",
                                            __tc.transient
                                                .storage
                                                .try_read()
                                                .map(|t| t
                                                    .contains::<Vec<UiNode>>(ResourceKey::root()))
                                                .unwrap_or_default()
                                        );

                                        if let Some(mut _nodes) = __tc
                                            .transient
                                            .storage
                                            .clone()
                                            .try_write()
                                            .expect("should be the owner")
                                            .take_resource::<Vec<UiNode>>(ResourceKey::root())
                                        {
                                            imgui.user_tool_nodes.append(&mut _nodes);
                                        }
                                    }
                                }
                            }
                        }
                    });
            }
        })
    }

    /// Enables the demo window,
    ///
    pub fn with_ui_node(mut self, ui_node: UiNode) -> Self {
        self.user_tool_nodes.push(ui_node);
        self
    }

    /// Adds an auxilary node,
    ///
    pub fn with_aux_node(
        mut self,
        aux_ui_id: impl Into<String>,
        aux_ui: impl FnMut(&mut EngineHandle, &mut ImguiMiddleware, &mut bool, &Ui)
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.tool_nodes.push((
            aux_ui_id.into(),
            ToolUiNode {
                engine_handle: None,
                show_ui: Arc::new(RwLock::new(aux_ui)),
                can_show: true,
            },
        ));
        self
    }

    fn show_tool_nodes(&mut self, ui: &Ui) {
        let mut nodes: Vec<(String, ToolUiNode)> = self.tool_nodes.drain(..).collect();

        for (_, auxnode) in nodes.iter_mut() {
            if auxnode.engine_handle.is_none() {
                auxnode.engine_handle = Some(
                    self.engine
                        .get()
                        .cloned()
                        .expect("should have an engine handle by this point"),
                );
            }

            auxnode.show(self, &ui);
        }

        self.tool_nodes = nodes.drain(..).collect();
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
    }

    fn on_user_event(&mut self, _user: &EnginePacket, _context: &crate::desktop::DesktopContext) {}

    fn on_window_redraw(
        &mut self,
        _: winit::window::WindowId,
        context: &crate::desktop::DesktopContext,
    ) {
        if let (Some(mut im_context), Some(mut platform)) =
            (self.context.take(), self.platform.take())
        {
            let io = im_context.io_mut();
            if let Ok(_) = platform.prepare_frame(io, context.window) {
                let ui = im_context.new_frame();

                if let Some(open_demo_window) = self.open_demo.as_mut() {
                    ui.show_demo_window(open_demo_window);
                }

                let mut formatter = UiFormatter {
                    rk: ResourceKey::root(),
                    imgui: ui,
                    subcommand: None,
                    tc: std::sync::Mutex::new(OnceLock::new()),
                    disp: None,
                    eh: std::sync::Mutex::new(
                        self.engine
                            .get()
                            .cloned()
                            .expect("should be bound to an engine"),
                    ),
                    frame_updates: RefCell::new(FrameUpdates::default()),
                };

                // ----------------------------------------
                //        **UI DISPLAY ENTRYPOINT**
                // ----------------------------------------
                if let Err(err) = self.fmt(&mut formatter) {
                    ui.text(format!("{err}"));
                }

                platform.prepare_render(&ui, context.window);
            }

            self.context
                .set(im_context)
                .expect("should have taken in the same function");
            self.platform
                .set(platform)
                .expect("should have taken in the same function");
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
    /// Pushes a new ui node to transient storage,
    ///
    /// **Note** When this fn is called it will take a snapshot of the current context.
    ///
    fn push_ui_node(
        &self,
        show: impl for<'a, 'b> Fn(&'a UiFormatter<'_>) -> bool + Send + Sync + 'static,
    );

    /// Pushes a new ui type node to transient storage,
    ///
    /// **Note** When this fn is called it will take a snapshot of the current context.
    ///
    async fn push_ui_type_node<G: Default + Send + Sync + 'static>(
        &self,
        show: impl for<'a, 'b> Fn(&'a UiFormatter<'b>) -> bool + Send + Sync + 'static,
    );
}

#[async_trait]
impl ImguiExt for ThunkContext {
    fn push_ui_node(
        &self,
        show: impl for<'a, 'b> Fn(&'a UiFormatter<'_>) -> bool + Send + Sync + 'static,
    ) {
        let mut storage = self
            .transient
            .storage
            .try_write()
            .expect("should only be called during transient code");

        let mut nodes = storage.maybe_put_resource(vec![], self.attribute.transmute());
        nodes.push(UiNode {
            show_ui_node: Some(Arc::new(show)),
            context: self.clone(),
        });
    }

    async fn push_ui_type_node<G: Default + Send + Sync + 'static>(
        &self,
        show: impl for<'a, 'b> Fn(&'a UiFormatter<'b>) -> bool + Send + Sync + 'static,
    ) {
        let mut storage = self
            .transient
            .storage
            .try_write()
            .expect("should only be called during transient code");

        let mut nodes = storage.maybe_put_resource(vec![], self.attribute.transmute());
        nodes.push(UiTypeNode {
            show_ui_node: Some(Arc::new(show)),
            dispatcher: self.dispatcher::<G>().await.transmute(),
        });
    }
}

/// Type-alias for an engine handle based UI function signature,
///
pub type ToolUi = Arc<
    RwLock<
        dyn FnMut(&mut EngineHandle, &mut ImguiMiddleware, &mut bool, &Ui) + Sync + Send + 'static,
    >,
>;

pub type ShowUiNode = Arc<dyn for<'frame> Fn(&UiFormatter<'frame>) -> bool + Sync + Send + 'static>;

/// UI Node contains a rendering function w/ a thunk context,
///
#[derive(Clone)]
pub struct UiTypeNode {
    /// Dispatcher for this ui node,
    ///
    pub dispatcher: Dispatcher<Shared, Attribute>,
    /// Function to show ui node,
    ///
    pub show_ui_node: Option<ShowUiNode>,
}

/// UI Node contains a rendering function w/ a thunk context,
///
#[derive(Clone)]
pub struct UiNode {
    /// Dispatcher for this ui node,
    ///
    pub context: ThunkContext,
    /// Funtion to show ui node,
    ///
    pub show_ui_node: Option<ShowUiNode>,
}

/// Tool UI node, containing a rendering function w/ engine handle,
///
#[derive(Clone)]
pub struct ToolUiNode {
    /// Engine handle,
    ///
    pub engine_handle: Option<EngineHandle>,
    /// Function to show ui,
    ///
    pub show_ui: ToolUi,
    /// True if the the node can be shown,
    ///
    pub can_show: bool,
}

impl ToolUiNode {
    /// Shows the ui tool,
    ///
    pub fn show(&mut self, imgui: &mut ImguiMiddleware, ui: &Ui) {
        if !self.can_show {
            return;
        }

        if let (Some(handle), Ok(mut show)) =
            (self.engine_handle.as_mut(), self.show_ui.try_write())
        {
            show(handle, imgui, &mut self.can_show, ui);
        }
    }

    /// Show the UI w/ a different engine handle,
    ///
    /// **Note** When created an aux ui node receives it's own engine handle. This allows
    /// passing a handle directly, such as the middleware's handle.
    ///
    pub fn show_with(
        &mut self,
        engine_handle: &mut EngineHandle,
        imgui: &mut ImguiMiddleware,
        ui: &Ui,
    ) {
        if let Ok(mut show) = self.show_ui.try_write() {
            show(engine_handle, imgui, &mut self.can_show, ui);
        }
    }
}

impl UiDisplayMut for UiNode {
    fn fmt(&mut self, ui: &UiFormatter<'_>) -> anyhow::Result<()> {
        if let Some(show_ui_node) = self.show_ui_node.as_ref() {
            show_ui_node(ui);
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl UiDisplayMut for UiTypeNode {
    fn fmt(&mut self, ui: &UiFormatter<'_>) -> anyhow::Result<()> {
        if let Some(show_ui_node) = self.show_ui_node.as_ref() {
            show_ui_node(ui);
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl UiDisplayMut for ImguiMiddleware {
    fn fmt(&mut self, ui: &UiFormatter<'_>) -> anyhow::Result<()> {
        let _ui = &ui.imgui;

        _ui.main_menu_bar(|| {
            _ui.menu("Tools", || {
                for (id, node) in self.tool_nodes.iter_mut() {
                    _ui.menu_item_config(id).build_with_ref(&mut node.can_show);
                }
            });
        });

        self.show_tool_nodes(_ui);

        if let Some(eh) = self.engine.get_mut() {
            if let Some(bg) = eh.background() {
                // Render ui nodes
                for ui_node in self.user_tool_nodes.iter_mut() {
                    // Swap current thunk context
                    let yielding = if let Ok(mut tc) = ui.tc.lock() {
                        let yielding = tc.take();
                        // ui.decorations.write().unwrap().take();
                        let current = ui_node.context.clone();
                        tc.set(current).ok();
                        yielding
                    } else {
                        None
                    };

                    // Format UI
                    ui_node.fmt(ui)?;

                    if let Ok(mut tc) = ui.tc.lock() {
                        ui_node.context = tc.take().expect("should have been set previously");

                        // Restore the previous thunk context
                        if let Some(yielding) = yielding {
                            // ui.decorations.write().unwrap().take();
                            tc.set(yielding).ok();
                        }
                    }
                }

                // Render ui-type nodes
                // for ui_type_node in self.ui_type_nodes.iter_mut() {
                //     ui.disp = Some(ui_type_node.dispatcher.clone());
                //     ui_type_node.fmt(ui)?;
                // }
                // ui.disp = None;

                // Initialize list of published resources on start-up
                if let Ok(mut _bg) = bg.call("engine://default/list/loopio.published") {
                    match _bg.status() {
                        loopio::background_work::CallStatus::Enabled => {
                            // TODO -- This is a change signal
                            if self.published.is_none() {
                                _bg.spawn();
                            }
                        }
                        loopio::background_work::CallStatus::Pending => {
                            let mut __tc = _bg.into_foreground().unwrap();

                            if let Ok(mut storage) = __tc.transient.clone().storage.try_write() {
                                self.published =
                                    Some(Published::default().unpack(storage.deref_mut()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
}
