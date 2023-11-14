use std::cell::OnceCell;
use std::sync::Arc;
use std::time::Instant;

use futures::pin_mut;
use futures::StreamExt;
use imgui::Ui;
use imgui_wgpu::RendererConfig;
use imgui_winit_support::WinitPlatform;
use tokio::sync::RwLock;
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
    /// Unused,
    ///
    __t: PhantomData<T>,
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
            ui_type_nodes: vec![],
            __update_start: None,
            __last_updated: None,
            __aux_ui: vec![],
            __t: PhantomData,
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
        /*
        Aux tool ideas
        - The aux tools are based on an engine handle, which basically has access to everything
        - So some generic tooling widgets could be helpful
        -- Tool Idea: Future monitor -- 
            -- Visualizing futures could be helpful in general, especially w/ view status of an EngineHandle
            -- 
        -- Tool Idea: 
        */

        self.with_aux_node(|handle, ui| {
            ui.window("aux-demo").size([600.0, 400.0], imgui::Condition::Appearing).build(move || {
                ui.text("Operations --");
                for (idx, (op, __op)) in handle.operations.iter_mut().enumerate() {
                    ui.label_text("Name", op);
                    
                    if !__op.is_running() {
                        if ui.button(format!("Start##{idx}")) {
                            __op.spawn();
                        }
                    } else if __op.is_finished() {
                        let result = __op.block_result();
                        eprintln!("success: {}", result.is_ok());
                    } else {
                        ui.text("Running");
                    }

                    ui.indent();
                    if let Some(context) = __op.context() {
                        if let Ok(s) = context.node.storage.try_read() {
                            if let Some(attributes) = s.resource::<ParsedAttributes>(None) {
                                if !attributes.paths.is_empty() {
                                    ui.text("Paths:");
                                    for (p, a) in attributes.paths.iter() {
                                        ui.label_text(format!("{}", a.key()), p);

                                        if let Some(defined) = attributes.properties.defined.get(a) {
                                            ui.label_text("# of properties", defined.len().to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ui.unindent();
                    ui.separator();
                    ui.new_line();
                }

                // Engine Handle Controls
                if !handle.is_running() {
                    if ui.button("Sync") {
                        if let Some(started) =
                            handle.spawn(|e| tokio::spawn(async move { e.sync().await }))
                        {
                            handle
                                .cache
                                .put_resource(started, Some(ResourceKey::with_hash("sync_command")))
                        }
                    }
                } else if let Some(true) = handle.is_finished() {
                    if let Some(started) = handle
                        .cache
                        .take_resource::<Instant>(Some(ResourceKey::with_hash("sync_command")))
                    {
                        if let Ok(finished) = handle.wait_for_finish(*started) {
                            *handle = finished;
                        } else {
                            // Remember to put this back
                            handle
                                .cache
                                .put_resource(started, Some(ResourceKey::with_hash("sync_command")))
                        }
                    }
                } else if ui.button("cancel") {
                    handle.cancel();
                }
            });
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
    /// TODO: Make this called more lazily
    /// 
    pub fn update(&mut self) {
        if let Some(engine) = self.engine.get_mut() {
            if !engine.is_running() {
                if let Some(last_updated) = self.__last_updated.as_ref() {
                    // TODO: Can remove when it's on demand
                    if last_updated.elapsed().as_secs() <= 10 {
                        return;
                    }
                    self.__last_updated.take();
                }

                if let Some(started) = engine.spawn(|mut e| {
                    tokio::spawn(async move {
                        let nodes = {
                            let nodes = e.scan_take_nodes::<UiNode>();
                            pin_mut!(nodes);

                            nodes.collect::<Vec<_>>().await
                        };

                        if !nodes.is_empty() {
                            let e = &mut e;
                            e.cache.put_resource(nodes, None);
                        }

                        Ok(e)
                    })
                }) {
                    self.__update_start = Some(started);
                }
            } else if let Some(true) = engine.is_finished() {
                if let Some(_wait_for_finish) = self.__update_start.take() {
                    if let Ok(mut r) = engine.wait_for_finish(_wait_for_finish) {
                        if let Some(nodes) = r.cache.take_resource::<Vec<UiNode>>(None) {
                            // TODO -- Should clear?
                            self.ui_nodes.clear();

                            self.ui_nodes.extend(*nodes);
                        }
                        self.__last_updated = Some(Instant::now());
                    }
                }
            }
        }
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

        // TOOD: This could be placed on a better handler to reduce overhead
        self.update();
    }

    fn on_user_event(&mut self, _user: &T, _context: &crate::desktop::DesktopContext<T>) {
        self.update();
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

                // TODO: Handle the output of show.

                for uinode in self.ui_nodes.iter_mut() {
                    uinode.show(&ui);
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

                    auxnode.show(&ui);
                }

                for ui_type_node in self.ui_type_nodes.iter_mut() {
                    ui_type_node.show(&ui);
                }

                platform.prepare_render(&ui, context.window);
            }
        }
    }
}

impl<T: 'static> ControlBus for ImguiMiddleware<T> {
    fn bind(&mut self, engine: EngineHandle) {
        {
            let stream = engine.scan_take_nodes::<UiNode>();
            pin_mut!(stream);

            let mut stream = futures::executor::block_on_stream(stream);

            while let Some(node) = stream.next() {
                self.ui_nodes.push(node);
            }
        }

        self.engine.set(engine).expect("should only be called once");
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
        show: impl for<'a, 'b> Fn(&'a mut Dispatcher<Shared, Attribute>, &'b Ui) -> bool + Send + Sync + 'static,
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
                .put_resource(ui_node, self.attribute.map(|a| a.transmute()))
        };
    }

    async fn add_ui_type_node<G: Default + Send + Sync + 'static>(
        &self,
        show: impl for<'a, 'b> Fn(&'a mut Dispatcher<Shared, Attribute>, &'b Ui) -> bool + Send + Sync + 'static,
    ) {
        let ui_node = UiTypeNode {
            show_ui: Some(Arc::new(show)),
            dispatcher: self.initialized_dispatcher::<G>().await.transmute(),
        };

        unsafe {
            self.node_mut()
                .await
                .put_resource(ui_node, self.attribute.map(|a| a.transmute()))
        };
    }
}

/// Type-alias for a dispatcher based UI signature,
/// 
pub type ShowTypeUi = Arc<dyn Fn(&mut Dispatcher<Shared, Attribute>, &Ui) -> bool + Sync + Send + 'static>;

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
    pub fn show(&mut self, ui: &Ui) -> bool{
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
        if let Ok(mut show) =
            self.show_ui.try_write()
        {
            show(engine_handle, ui)
        } else {
            false
        }
    }
}
