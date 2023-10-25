use winit::event_loop::{self};
use winit::window::WindowId;
use winit::event::DeviceId;
use winit::event::DeviceEvent;
use winit::event::WindowEvent;
use winit::event::StartCause;
use winit::event_loop::EventLoopWindowTarget;

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
}

impl<T: 'static> Desktop<T> {
    /// Creates a new window,
    ///
    pub fn new() -> anyhow::Result<Desktop<T>> {
        let event_loop = event_loop::EventLoopBuilder::with_user_event().build()?;
        Ok(Desktop {
            event_loop_proxy: event_loop.create_proxy(),
            event_loop: event_loop.into(),
        })
    }

    /// Starts the window event loop and creates a new Window,
    /// 
    pub fn open<D: DesktopApp<T>>(mut self, mut app: D) {
        if let Some(event_loop) = self.event_loop.take() {
            if let Ok(window) = winit::window::Window::new(&event_loop) {
                let _ = app.configure(window);

                let _ = event_loop.run(move |event, window_target| {
                    app.before_event(window_target);

                    match event {
                        winit::event::Event::NewEvents(start_cause) => app.on_new_events(start_cause),
                        winit::event::Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                            window_target.exit();
                        },
                        winit::event::Event::WindowEvent { window_id, event } => app.on_window_event(window_id, event),
                        winit::event::Event::DeviceEvent { device_id, event } => app.on_device_event(device_id, event),
                        winit::event::Event::UserEvent(user_event) => app.on_user_event(user_event),
                        winit::event::Event::Suspended => app.on_suspended(),
                        winit::event::Event::Resumed => app.on_resumed(),
                        winit::event::Event::AboutToWait => app.on_about_to_wait(),
                        winit::event::Event::LoopExiting => app.on_loop_exiting(),
                        winit::event::Event::MemoryWarning => app.on_memory_warning(),
                    }

                    app.after_event(window_target);
                });
            }
        }
    }
}

/// Winit based event loop handler for desktop apps,
/// 
#[allow(unused_variables)]
pub trait DesktopApp<T: 'static> {
    /// Called to configure the window,
    /// 
    fn configure(&self, window: winit::window::Window) -> winit::window::Window {
        window
    }

    /// Called before an event in the event loop is handled,
    /// 
    fn before_event(&mut self, window: &EventLoopWindowTarget<T>) {

    }

    /// Called on `winit::event::Event::NewEvents`,
    /// 
    fn on_new_events(&mut self, start_cause: StartCause) {
        
    }

    /// Called on `winit::event::Event::WindowEvent`
    /// 
    fn on_window_event(&mut self, window_id: WindowId, window_event: WindowEvent) {

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
    fn after_event(&mut self, window: &EventLoopWindowTarget<T>) {

    }
}

/// Graphics pipeline 
/// 
pub struct GraphicsPipeline {

}

impl<T: 'static> DesktopApp<T> for GraphicsPipeline {
    fn on_new_events(&mut self, start_cause: StartCause) {
        match start_cause {
            StartCause::ResumeTimeReached { start, requested_resume } => todo!(),
            StartCause::WaitCancelled { start, requested_resume } => todo!(),
            StartCause::Poll => todo!(),
            StartCause::Init => todo!(),
        }
    }
}