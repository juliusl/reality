use loopio::engine::Engine;
use tracing::error;
use tracing::info;
use winit::window::Window;
use winit::window::WindowId;
use winit::event::DeviceId;
use winit::event::DeviceEvent;
use winit::event::WindowEvent;
use winit::event::StartCause;
use winit::event_loop::EventLoopBuilder;
use winit::event_loop::EventLoopWindowTarget;

use crate::Controller;
use crate::BackgroundWork;
use crate::controller::ControlBus;

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
}

impl<T: 'static> Desktop<T> {
    /// Creates a new window,
    ///
    pub fn new() -> anyhow::Result<Desktop<T>> {
        let event_loop = EventLoopBuilder::with_user_event().build()?;
        Ok(Desktop {
            event_loop_proxy: event_loop.create_proxy(),
            event_loop: event_loop.into(),
            // project_loop: None,
        })
    }

    /// Starts the window event loop and creates a new Window,
    /// 
    pub fn open<D: DesktopApp<T>>(mut self, mut app: D) {
        if let Some(event_loop) = std::cell::OnceCell::take(&mut self.event_loop) {
            if let Ok(window) = winit::window::Window::new(&event_loop) {
                let window = app.configure_window(window);
                info!("Starting window");
                
                app.before_event_loop(&window);

                let _ = event_loop.run(move |event, window_target| {
                    let desktop = DesktopContext{ window: &window, event_loop_target: window_target};
                    app.before_event(&desktop);

                    match event {
                        winit::event::Event::NewEvents(start_cause) => app.on_new_events(start_cause, &desktop),
                        winit::event::Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                            window_target.exit();
                        },             
                        winit::event::Event::WindowEvent { event: WindowEvent::RedrawRequested, window_id } => {
                            window.pre_present_notify();
                            app.on_window_redraw(window_id, &desktop);
                            app.on_window_event(window_id, WindowEvent::RedrawRequested, &desktop);
                        },
                        winit::event::Event::WindowEvent { window_id, event } => app.on_window_event(window_id, event, &desktop),
                        winit::event::Event::DeviceEvent { device_id, event } => app.on_device_event(device_id, event, &desktop),
                        winit::event::Event::UserEvent(user_event) => app.on_user_event(user_event, &desktop),
                        winit::event::Event::Suspended => app.on_suspended(&desktop),
                        winit::event::Event::Resumed => app.on_resumed(&desktop),
                        winit::event::Event::AboutToWait => {
                            app.on_about_to_wait(&desktop);
                            window.request_redraw();
                        },
                        winit::event::Event::LoopExiting => app.on_loop_exiting(&desktop),
                        winit::event::Event::MemoryWarning => app.on_memory_warning(&desktop),
                    }

                    app.after_event(&desktop);
                });
            } else {
                error!("Could not open window");
            }
        }
    }
}

impl<T: 'static, A: DesktopApp<T>> Controller<A> for Desktop<T> {
    fn take_control(self, engine: Engine) -> BackgroundWork {
        let app = A::create(engine);

        self.open(app);

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
    pub event_loop_target: &'a EventLoopWindowTarget<UserEvent>
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
    fn before_event_loop(&mut self, window: &Window) {

    }

    /// Called before an event in the event loop is handled,
    /// 
    fn before_event(&mut self, context: &DesktopContext<T>) {

    }

    /// Called on `winit::event::Event::NewEvents`,
    /// 
    fn on_new_events(&mut self, start_cause: StartCause, context: &DesktopContext<T>) {
        
    }

    /// Called on `winit::event::Event::WindowEvent`
    /// 
    fn on_window_event(&mut self, window_id: WindowId, event: WindowEvent, context: &DesktopContext<T>) {

    }

    /// Called before `winit::event::Event::WindowEvent`
    /// 
    fn on_window_redraw(&mut self, window_id: WindowId, context: &DesktopContext<T>) {

    }

    /// Called on `winit::event::Event::DeviceEvent`
    /// 
    fn on_device_event(&mut self, window_id: DeviceId, window_event: DeviceEvent, context: &DesktopContext<T>) {

    }

    /// Called on `winit::event::Event::UserEvent`
    /// 
    fn on_user_event(&mut self, user: T, context: &DesktopContext<T>) {

    }

    /// Called on `winit::event::Event::Suspended`
    /// 
    fn on_suspended(&mut self, context: &DesktopContext<T>) {

    }

    /// Called on `winit::event::Event::Resumed`
    /// 
    fn on_resumed(&mut self, context: &DesktopContext<T>) {

    }

    /// Called on `winit::event::Event::AboutToWait`
    /// 
    fn on_about_to_wait(&mut self, context: &DesktopContext<T>) {

    }

    /// Called on `winit::event::Event::LoopExiting`
    /// 
    fn on_loop_exiting(&mut self, context: &DesktopContext<T>) {

    }

    /// Called on `winit::event::Event::MemoryWarning`
    /// 
    fn on_memory_warning(&mut self, context: &DesktopContext<T>) {

    }

    /// Called after the event_loop event is handled,
    /// 
    fn after_event(&mut self, context: &DesktopContext<T>) {

    }
}
