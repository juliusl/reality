use reality::StorageTarget;
use winit::window::Window;
use winit::window::WindowId;
use winit::event::DeviceId;
use winit::event::DeviceEvent;
use winit::event::WindowEvent;
use winit::event::StartCause;
use winit::event_loop::EventLoopBuilder;
use winit::event_loop::EventLoopWindowTarget;

use crate::project_loop::AppType;
use crate::project_loop::ProjectLoop;
use crate::project_loop::InteractionLoop;

/// Desktop,
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

    // pub project_loop: Option<ProjectLoop<S>>
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
    pub fn open<S: StorageTarget + 'static, D: DesktopApp<T, S>>(mut self, mut app: D) {
        if let Some(event_loop) = std::cell::OnceCell::take(&mut self.event_loop) {
            if let Ok(window) = winit::window::Window::new(&event_loop) {
                let window = app.configure_window(window);
                println!("Starting window");
                
                let _ = event_loop.run(move |event, window_target| {
                    app.before_event(window_target, &window);

                    match event {
                        winit::event::Event::NewEvents(start_cause) => app.on_new_events(start_cause),
                        winit::event::Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                            window_target.exit();
                        },             
                        winit::event::Event::WindowEvent { event: WindowEvent::RedrawRequested, window_id } => {
                            window.pre_present_notify();
                            app.on_window_redraw(window_id, window_target, &window);
                            app.on_window_event(window_id, WindowEvent::RedrawRequested, &window);
                        },
                        winit::event::Event::WindowEvent { window_id, event } => app.on_window_event(window_id, event, &window),
                        winit::event::Event::DeviceEvent { device_id, event } => app.on_device_event(device_id, event),
                        winit::event::Event::UserEvent(user_event) => app.on_user_event(user_event),
                        winit::event::Event::Suspended => app.on_suspended(),
                        winit::event::Event::Resumed => app.on_resumed(),
                        winit::event::Event::AboutToWait => {
                            app.on_about_to_wait();
                            window.request_redraw();
                        },
                        winit::event::Event::LoopExiting => app.on_loop_exiting(),
                        winit::event::Event::MemoryWarning => app.on_memory_warning(),
                    }

                    app.after_event(window_target, &window);
                });
            } else {
                println!("Could not open window");
            }
        }
    }
}

impl<T: 'static, S: StorageTarget + 'static, A: DesktopApp<T, S>> InteractionLoop<S, A> for Desktop<T> {
    fn take_control(self, project_loop: ProjectLoop<S>) {
        let app = A::create(project_loop);

        self.open(app);
    }
}

/// Winit based event loop handler for desktop apps,
/// 
#[allow(unused_variables)]
pub trait DesktopApp<T: 'static, S: StorageTarget + 'static>: AppType<S> {
    /// Called to configure the window,
    /// 
    fn configure_window(&self, window: winit::window::Window) -> winit::window::Window {
        window
    }

    /// Called before an event in the event loop is handled,
    /// 
    fn before_event(&mut self, window_target: &EventLoopWindowTarget<T>, window: &Window) {

    }

    /// Called on `winit::event::Event::NewEvents`,
    /// 
    fn on_new_events(&mut self, start_cause: StartCause) {
        
    }

    /// Called on `winit::event::Event::WindowEvent`
    /// 
    fn on_window_event(&mut self, window_id: WindowId, window_event: WindowEvent, window: &Window) {

    }

    /// Called before `winit::event::Event::WindowEvent`
    /// 
    fn on_window_redraw(&mut self, window_id: WindowId, window_target: &EventLoopWindowTarget<T>, window: &Window) {

    }

    /// Called on `winit::event::Event::DeviceEvent`
    /// 
    fn on_device_event(&mut self, window_id: DeviceId, window_event: DeviceEvent) {

    }

    /// Called on `winit::event::Event::UserEvent`
    /// 
    fn on_user_event(&mut self, user: T) {

    }

    /// Called on `winit::event::Event::Suspended`
    /// 
    fn on_suspended(&mut self) {

    }

    /// Called on `winit::event::Event::Resumed`
    /// 
    fn on_resumed(&mut self) {

    }

    /// Called on `winit::event::Event::AboutToWait`
    /// 
    fn on_about_to_wait(&mut self) {

    }

    /// Called on `winit::event::Event::LoopExiting`
    /// 
    fn on_loop_exiting(&mut self) {

    }

    /// Called on `winit::event::Event::MemoryWarning`
    /// 
    fn on_memory_warning(&mut self) {

    }

    /// Called after the event_loop event is handled,
    /// 
    fn after_event(&mut self, window_target: &EventLoopWindowTarget<T>, window: &Window) {

    }
}
