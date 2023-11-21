use std::process::exit;

use loopio::engine::EngineBuilder;
use tracing::error;
use tracing::info;

use winit::event::DeviceEvent;
use winit::event::DeviceId;
use winit::event::Event;
use winit::event::StartCause;
use winit::event::WindowEvent;
use winit::event_loop::EventLoopBuilder;
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;
use winit::window::WindowId;
use winit::event_loop::EventLoopProxy;
use winit::dpi::LogicalSize;

use loopio::prelude::*;

pub mod winit {
    #[cfg(feature = "desktop-imgui")]
    pub use winit_27::*;
    #[cfg(feature = "desktop-vnext")]
    pub use winit_29::*;
}

use crate::controller::ControlBus;
use crate::BackgroundWork;
use crate::Controller;
use crate::ext::imgui_ext::ImguiExt;

/// Desktop app that enables developer utilities,
/// 
/// - Enables the engine packet listener on app launch,
/// - Enables developer widget plugins on the loopio engine
/// 
pub struct DevProject;

impl DevProject {
    /// Creates a new project based on the current directory,
    /// 
    pub fn current_project(self) -> (Desktop<()>, EngineBuilder) {
        self.open_project(CurrentDir.workspace())
    }

    /// Opens a project from a workspace,
    /// 
    pub fn open_project(self, mut workspace: Workspace) -> (Desktop<()>, EngineBuilder) {
        (Desktop::<()>::new().expect("should be able to create a new desktop app").with_title("Dev Project App").enable_engine_packet_listener(), {
            let mut engine = Engine::builder();
            engine.enable::<crate::widgets::FrameEditor>();
            engine.enable::<Placeholder>();
            workspace.add_buffer("dev_project_frame_editor.md", 
            r#"
            ```runmd
            + .operation open_debug_window
            # -- Opens a debug window
            <nebudeck.placeholder>
            ```
            "#);
            engine.set_workspace(workspace);
            engine
        })
    }
}

/// Placeholder plugin,
/// 
#[derive(Reality, Clone, Debug, Default)]
#[reality(call = placeholder, plugin)]
pub struct Placeholder {
    #[reality(derive_fromstr)]
    name: String
}

async fn placeholder(tc: &mut ThunkContext) -> anyhow::Result<()> {
    tc.print_debug_info();
    tc.add_ui_node(|tc, ui| {
        
        true
    }).await;
    Ok(())
}
/// This controller provides access to winit Windowing system and event loop,
///
/// This controller can be used to build a desktop app.
///
pub struct Desktop<T>
where
    T: 'static,
{
    /// Winit event loop,
    ///
    pub event_loop: std::cell::OnceCell<winit::event_loop::EventLoop<T>>,
    /// Event loop proxy,
    ///
    pub event_loop_proxy: winit::event_loop::EventLoopProxy<T>,
    /// If true, will enable the engine packet listener delegated
    /// to this controller.
    ///
    pub enable_engine_packet_listener: bool,
    /// Starting resolution size, Defaults to 1920x1080.
    /// 
    /// **Note** This will be overridden if set by middleware.
    /// 
    pub starting_resolution: (f64, f64),
    /// Title of the window when starting up, Default to empty.
    /// 
    /// **Note** This will be overridden if set by middleware.
    /// 
    pub window_title: String,
}

impl<T: 'static> Desktop<T> {
    /// Creates a new window,
    ///
    pub fn new() -> anyhow::Result<Desktop<T>> {
        #[cfg(feature = "desktop-vnext")]
        let event_loop = EventLoopBuilder::with_user_event().build()?;

        #[cfg(feature = "desktop-imgui")]
        let event_loop = EventLoopBuilder::with_user_event().build();
        Ok(Desktop {
            event_loop_proxy: event_loop.create_proxy(),
            event_loop: event_loop.into(),
            enable_engine_packet_listener: false,
            starting_resolution: (1920.0, 1080.0),
            window_title: String::new(),
            // project_loop: None,
        })
    }

    /// Enables the engine packet listener,
    ///
    /// The engine packet listener is required for engine packets to be received and handled
    /// by the engine.
    ///
    pub fn enable_engine_packet_listener(mut self) -> Self {
        self.enable_engine_packet_listener = true;
        self
    }

    /// Sets a starting resolution for the window,
    /// 
    /// **Note**: This can be overidden by middleware.
    /// 
    pub fn with_resolution(mut self, height: impl Into<f64>, width: impl Into<f64>) -> Self {
        self.starting_resolution = (width.into(), height.into());
        self
    }

    /// Sets a starting window title for the window,
    /// 
    /// **Note**: This can be overidden by middleware.
    /// 
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.window_title = title.into();
        self
    }

    /// Starts the window event loop and creates a new Window,
    ///
    pub fn open<D: DesktopApp<T> + 'static>(mut self, mut app: D) {
        if let Some(event_loop) = std::cell::OnceCell::take(&mut self.event_loop) {
            if let Ok(window) = winit::window::Window::new(&event_loop) {
                window.set_inner_size(LogicalSize::new(self.starting_resolution.0, self.starting_resolution.1));
                window.set_title(&self.window_title);
                let window = app.configure_window(window);
                info!("Starting window");

                app.before_event_loop(&window, self.event_loop_proxy.clone());

                #[cfg(feature = "desktop-imgui")]
                let _ = event_loop.run(move |event, window_target, _| {
                    Self::handle_event(event, &window, window_target, &mut app);
                });
                #[cfg(feature = "desktop-vnext")]
                let _ = event_loop.run(move |event, window_target| {
                    Self::handle_event(event, &window, window_target, &mut app)
                });
            } else {
                error!("Could not open window");
            }
        }
    }

    fn handle_event<D: DesktopApp<T> + 'static>(
        event: Event<T>,
        window: &Window,
        window_target: &EventLoopWindowTarget<T>,
        app: &mut D,
    ) {
        let desktop = DesktopContext {
            window,
            event_loop_target: window_target,
        };

        // **Note** MacOS will send events w/ invalid values, this is to ensure that these events cannot reach the pipeline
        if let Event::WindowEvent { event, .. } = &event {
            match event {
                WindowEvent::Resized(size) => {
                    if size.height >= u32::MAX || size.height >= u32::MAX {
                        return;
                    } else {
                        window.set_inner_size(*size);
                    }
                }
                WindowEvent::Moved(pos) => {
                    if pos.x >= i32::MAX || pos.y >= i32::MAX {
                        return;
                    }
                }
                WindowEvent::ScaleFactorChanged {
                    ref new_inner_size, ..
                } => {
                    if new_inner_size.height >= u32::MAX || new_inner_size.width >= u32::MAX {
                        return;
                    }
                }
                _ => {}
            }
        }
        app.before_event(&event, &desktop);

        match event {
            winit::event::Event::NewEvents(start_cause) => app.on_new_events(start_cause, &desktop),
            winit::event::Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                exit(0);
                // window_target.exit();
            }
            #[cfg(feature = "desktop-vnext")]
            winit::event::Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                window_id,
            } => {
                window.pre_present_notify();
                app.on_window_redraw(window_id, &desktop);
                app.on_window_event(window_id, WindowEvent::RedrawRequested, &desktop);
            }
            winit::event::Event::WindowEvent { window_id, event } => {
                app.on_window_event(window_id, &event, &desktop)
            }
            winit::event::Event::DeviceEvent { device_id, event } => {
                app.on_device_event(device_id, &event, &desktop)
            }
            winit::event::Event::UserEvent(user_event) => app.on_user_event(&user_event, &desktop),
            winit::event::Event::Suspended => app.on_suspended(&desktop),
            winit::event::Event::Resumed => app.on_resumed(&desktop),
            #[cfg(feature = "desktop-vnext")]
            winit::event::Event::AboutToWait => {
                app.on_about_to_wait(&desktop);
                window.request_redraw();
            }
            #[cfg(feature = "desktop-vnext")]
            winit::event::Event::LoopExiting => app.on_loop_exiting(&desktop),
            #[cfg(feature = "desktop-vnext")]
            winit::event::Event::MemoryWarning => app.on_memory_warning(&desktop),

            #[cfg(feature = "desktop-imgui")]
            winit::event::Event::RedrawRequested(window_id) => {
                app.on_window_redraw(window_id, &desktop);
                // app.on_window_event(window_id, WindowEvent::RedrawRequested, &desktop);
            }
            #[cfg(feature = "desktop-imgui")]
            winit::event::Event::MainEventsCleared => {
                window.request_redraw();
            }
            #[cfg(feature = "desktop-imgui")]
            winit::event::Event::RedrawEventsCleared => {}
            #[cfg(feature = "desktop-imgui")]
            winit::event::Event::LoopDestroyed => {
                return;
            }
        }

        app.after_event(&desktop);
    }
}

impl<T: 'static, A: DesktopApp<T> + 'static> Controller<A> for Desktop<T> {
    fn take_control(self, mut app: Box<A>, engine: ForegroundEngine) -> BackgroundWork {
        app.bind(engine.engine_handle());

        self.open(*app);
        None
    }
}

/// Context during event loop w/ access to winit primitives,
///
pub struct DesktopContext<'a, UserEvent: 'static> {
    /// Winit Window handle,
    ///
    pub window: &'a Window,
    /// Winit event loop target handle,
    ///
    pub event_loop_target: &'a EventLoopWindowTarget<UserEvent>,
}

/// Winit based event loop handler for desktop apps,
///
#[allow(unused_variables)]
pub trait DesktopApp<T: 'static>: ControlBus {
    /// Called to configure the window,
    ///
    fn configure_window(&self, window: Window) -> winit::window::Window {
        window
    }

    /// Called before the event loop starts,
    ///
    /// Should be a reasonable place to initialize graphics pipeline,
    ///
    fn before_event_loop(&mut self, window: &Window, event_proxy: EventLoopProxy<T>) {}

    /// Called before an event in the event loop is handled,
    ///
    fn before_event(&mut self, event: &Event<T>, context: &DesktopContext<T>) {}

    /// Called on `winit::event::Event::NewEvents`,
    ///
    fn on_new_events(&mut self, start_cause: StartCause, context: &DesktopContext<T>) {}

    /// Called on `winit::event::Event::WindowEvent`
    ///
    fn on_window_event(
        &mut self,
        window_id: WindowId,
        event: &WindowEvent,
        context: &DesktopContext<T>,
    ) {
    }

    /// Called before `winit::event::Event::WindowEvent`
    ///
    fn on_window_redraw(&mut self, window_id: WindowId, context: &DesktopContext<T>) {}

    /// Called on `winit::event::Event::DeviceEvent`
    ///
    fn on_device_event(
        &mut self,
        window_id: DeviceId,
        window_event: &DeviceEvent,
        context: &DesktopContext<T>,
    ) {
    }

    /// Called on `winit::event::Event::UserEvent`
    ///
    fn on_user_event(&mut self, user: &T, context: &DesktopContext<T>) {}

    /// Called on `winit::event::Event::Suspended`
    ///
    fn on_suspended(&mut self, context: &DesktopContext<T>) {}

    /// Called on `winit::event::Event::Resumed`
    ///
    fn on_resumed(&mut self, context: &DesktopContext<T>) {}

    /// Called on `winit::event::Event::AboutToWait`
    ///
    fn on_about_to_wait(&mut self, context: &DesktopContext<T>) {}

    /// Called on `winit::event::Event::LoopExiting`
    ///
    fn on_loop_exiting(&mut self, context: &DesktopContext<T>) {}

    /// Called on `winit::event::Event::MemoryWarning`
    ///
    fn on_memory_warning(&mut self, context: &DesktopContext<T>) {}

    /// Called after the event_loop event is handled,
    ///
    fn after_event(&mut self, context: &DesktopContext<T>) {}
}
