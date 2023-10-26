use intrglctive::project_loop::AppType;
use intrglctive::project_loop::ProjectLoop;
use intrglctive::desktop::*;
use reality::StorageTarget;
use reality::Shared;
use reality::Project;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;
use winit::window::WindowId;

/// Minimal example for opening a blank window,
/// 
fn main() -> anyhow::Result<()> {
    let desktop = Desktop::<()>::new()?;

    BlankWindow::start_interaction(ProjectLoop::new(Project::new(Shared::default())), desktop);

    Ok(())
}

struct BlankWindow;

impl AppType<Shared> for BlankWindow {
    fn initialize_storage() -> Shared {
        Shared::default()
    }

    fn create(_: ProjectLoop<Shared>) -> Self {
        BlankWindow
    }
}

impl DesktopApp<(), Shared> for BlankWindow {
    fn configure_window(&self, window: winit::window::Window) -> winit::window::Window {
        // window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        window.set_title("Blank Window");
        window.set_visible(true);
        window.set_resizable(true);
        // window.set_maximized(true);
        // window.set_min_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0).into());
        window
    }

    fn on_window_redraw(&mut self, _: WindowId, _: &EventLoopWindowTarget<()>, window: &Window) {
        /// Copied from winit examples
        /// 
        fn fill_window(window: &Window) {
            use softbuffer::{Context, Surface};
            use std::cell::RefCell;
            use std::collections::HashMap;
            use std::mem::ManuallyDrop;
            use std::num::NonZeroU32;
        
            /// The graphics context used to draw to a window.
            struct GraphicsContext {
                /// The global softbuffer context.
                context: Context,
        
                /// The hash map of window IDs to surfaces.
                surfaces: HashMap<WindowId, Surface>,
            }
        
            impl GraphicsContext {
                fn new(w: &Window) -> Self {
                    Self {
                        context: unsafe { Context::new(w) }.expect("Failed to create a softbuffer context"),
                        surfaces: HashMap::new(),
                    }
                }
        
                fn surface(&mut self, w: &Window) -> &mut Surface {
                    self.surfaces.entry(w.id()).or_insert_with(|| {
                        unsafe { Surface::new(&self.context, w) }
                            .expect("Failed to create a softbuffer surface")
                    })
                }
            }
        
            thread_local! {
                // NOTE: You should never do things like that, create context and drop it before
                // you drop the event loop. We do this for brevity to not blow up examples. We use
                // ManuallyDrop to prevent destructors from running.
                //
                // A static, thread-local map of graphics contexts to open windows.
                static GC: ManuallyDrop<RefCell<Option<GraphicsContext>>> = ManuallyDrop::new(RefCell::new(None));
            }
        
            GC.with(|gc| {
                // Either get the last context used or create a new one.
                let mut gc = gc.borrow_mut();
                let surface = gc
                    .get_or_insert_with(|| GraphicsContext::new(window))
                    .surface(window);
        
                // Fill a buffer with a solid color.
                const DARK_GRAY: u32 = 0xFF181818;
                let size = window.inner_size();
        
                surface
                    .resize(
                        NonZeroU32::new(size.width).expect("Width must be greater than zero"),
                        NonZeroU32::new(size.height).expect("Height must be greater than zero"),
                    )
                    .expect("Failed to resize the softbuffer surface");
        
                let mut buffer = surface
                    .buffer_mut()
                    .expect("Failed to get the softbuffer buffer");
                buffer.fill(DARK_GRAY);
                buffer
                    .present()
                    .expect("Failed to present the softbuffer buffer");
            })
        }

        fill_window(window);
    }

    fn after_event(&mut self, window_target: &EventLoopWindowTarget<()>, _: &Window) {
        window_target.set_control_flow(ControlFlow::Poll);
    }
}

