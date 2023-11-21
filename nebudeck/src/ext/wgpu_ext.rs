use super::prelude::*;

use futures::executor::block_on;
use loopio::engine::EnginePacket;
use loopio::foreground::ForegroundEngine;
use loopio::prelude::Engine;
use loopio::prelude::EngineHandle;
use loopio::prelude::StorageTarget;
use loopio::prelude::ThunkContext;
use std::cell::OnceCell;
use std::ops::DerefMut;
use std::pin::Pin;
use tracing::trace;
use wgpu::util::StagingBelt;
use wgpu::Adapter;
use wgpu::BindGroup;
use wgpu::Buffer;
use wgpu::CommandEncoder;
use wgpu::Device;
use wgpu::IndexFormat;
use wgpu::InstanceDescriptor;
use wgpu::Queue;
use wgpu::RenderPass;
use wgpu::RenderPipeline;
use wgpu::Surface;
use wgpu::SurfaceConfiguration;
use wgpu::TextureView;
use winit_27::event_loop::EventLoopProxy;

use crate::desktop::DesktopApp;
use crate::BackgroundWork;
use crate::ControlBus;
use crate::Controller;

/// Adds extensions to ThunkContext for handling wgpu primitives as resources w/ during
/// thunk execution.
///
macro_rules! define_wgpu_resource_management {
    (
        $(#[$meta:meta])*
        pub trait $trait:ident {
            [$(
                $(#[doc = $doc:literal])*
                ($suffix:ident, $ty:ty)
            ),*
            ]
        }
    ) => {
    paste::paste!
    {
        $(#[$meta])*
        pub trait $trait {
            $(
                    $(#[doc = $doc])*
                    async fn [<set_ $suffix>](&mut self, resource: $ty);
                    $(#[doc = $doc])*
                    async fn [<take_ $suffix>](&mut self) -> Option<$ty>;
            )*
        }

        $(#[$meta])*
        impl $trait for ThunkContext {
            $(
                async fn [<set_ $suffix>](&mut self, instances: $ty) {
                    let mut transport = self.transient_mut().await;

                    transport.put_resource(instances, loopio::prelude::ResourceKey::root());
                }

                async fn [<take_ $suffix>](&mut self) -> Option<$ty> {
                    let mut transport = self.transient_mut().await;

                    return transport.take_resource(loopio::prelude::ResourceKey::root()).map(|r| *r);
                }
            )*
        }
    }
    };
}

define_wgpu_resource_management! {
    #[async_trait::async_trait]
    pub trait WgpuResourceManagementExt {
        [
            (command_encoder, CommandEncoder),
            /// Handle setting/taking wgpu RenderPipeline
            (render_pipeline, RenderPipeline),
            /// Handle setting/taking wgpu lighting RenderPipeline
            (lighting_pipeline, RenderPipeline),
            /// Handle settings/taking render pass index buffer,
            (index_buffer, Buffer),
            /// Handle setting/taking render pass instance buffer,
            (instance_buffer, Buffer),
            /// Handle setting/taking render pass vertex buffer,
            (vertex_buffer, Buffer),
            /// Handle setting/taking render pass camera buffer,
            (camera_buffer, Buffer),
            /// Handle setting/taking render pass camera buffer,
            (lighting_buffer, Buffer),
            /// Handles setting/taking render pass main bind group,
            (bind_group, BindGroup),
            /// Handles setting/taking render pass camera bind group,
            (camera_bind_group, BindGroup),
            /// Handles setting/taking render pass camera bind group,
            (lighting_bind_group, BindGroup),
            /// Handles setting/taking the render passes index format,
            (index_format, IndexFormat)
        ]
    }
}

#[async_trait::async_trait]
pub trait WgpuRenderExt {
    async fn add_render_stage(
        &mut self,
        stage: impl for<'a> Fn(Box<RenderPass<'a>>) -> Box<RenderPass<'a>> + Send + Sync + 'static,
    );

    async fn render(&mut self) -> anyhow::Result<()>;
}

pub struct RenderStages<'a> {
    stages: Vec<RenderStage<'a>>,
}

pub type RenderStage<'a> =
    Box<dyn Fn(Box<RenderPass<'a>>) -> Box<RenderPass<'a>> + Send + Sync + 'static>;

#[async_trait::async_trait]
impl WgpuRenderExt for ThunkContext {
    async fn add_render_stage(
        &mut self,
        stage: impl for<'a> Fn(Box<RenderPass<'a>>) -> Box<RenderPass<'a>> + Send + Sync + 'static,
    ) {
        if let Some(mut stages) = self
            .transient_mut()
            .await
            .resource_mut::<RenderStages>(loopio::prelude::ResourceKey::root())
        {
            stages.deref_mut().stages.push(Box::new(stage));
        } else {
        }
    }

    async fn render(&mut self) -> anyhow::Result<()> {
        let render_pass = self.transient_mut().await.take_resource::<RenderPass>(loopio::prelude::ResourceKey::root());

        if let Some(stages) = self.transient().await.resource::<RenderStages>(loopio::prelude::ResourceKey::root()) {
            if let Some(mut render_pass) = render_pass {
                for stage in stages.stages.iter() {
                    render_pass = stage(render_pass);
                }
            }
        } else {
        }
        Ok(())
    }
}

/// This system enables access to the system's gpu devices,
///
pub struct WgpuSystem {
    /// Handle to the compiled engine,
    ///
    pub engine: OnceCell<EngineHandle>,
    /// Handle to wgpu hardware,
    ///
    pub hardware: OnceCell<HardwareContext>,
    /// Staging belt,
    ///
    pub staging_belt: OnceCell<StagingBelt>,
    /// Rendering and event loop middleware,
    ///
    pub middleware: Vec<BoxedMiddleware>,
}

impl WgpuSystem {
    /// Creates a new wgpu system,
    ///
    pub const fn new() -> Self {
        WgpuSystem {
            engine: OnceCell::new(),
            hardware: OnceCell::new(),
            staging_belt: OnceCell::new(),
            middleware: vec![],
        }
    }

    /// Creates a new wgpu system w/ middleware,
    ///
    pub const fn with(middleware: Vec<BoxedMiddleware>) -> Self {
        WgpuSystem {
            engine: OnceCell::new(),
            hardware: OnceCell::new(),
            staging_belt: OnceCell::new(),
            middleware,
        }
    }

    /// Adds middleware to the system,
    ///
    pub fn add_middleware(
        &mut self,
        middleware: impl RenderPipelineMiddleware + Unpin + 'static,
    ) {
        self.middleware.push(Box::pin(middleware));
    }
}

/// Contains wgpu types,
///
#[derive(Debug)]
pub struct HardwareContext {
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    pub surface_config: SurfaceConfiguration,
    pub surface: Surface,
}

impl HardwareContext {
    /// Creates a new hardware context from a window handle,
    ///
    pub fn new(window: &winit::window::Window) -> anyhow::Result<HardwareContext> {
        let instance_desc = InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
        };
        let instance = wgpu::Instance::new(instance_desc);

        for adapter in instance.enumerate_adapters(wgpu::Backends::PRIMARY) {
            eprintln!("Available: {:?}", adapter);
        }

        let surface = unsafe { instance.create_surface(window)? };
        eprintln!(
            "Got surface: {:?} {:?} {:?}",
            surface,
            window.inner_size(),
            window.outer_size()
        );
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).unwrap();

        let inner_size = window.inner_size();
        if let Some(config) =
            surface.get_default_config(&adapter, inner_size.width, inner_size.height)
        {
            surface.configure(&device, &config);

            Ok(HardwareContext {
                adapter,
                device,
                queue,
                surface_config: config,
                surface,
            })
        } else {
            Err(anyhow::anyhow!("Could not get new hardware context"))
        }
    }
}

impl DesktopApp for WgpuSystem {
    fn configure_window(
        &self,
        window: winit::window::Window,
    ) -> crate::desktop::winit::window::Window {
        window.set_resizable(true);
        // window.set_maximized(true);
        self.hardware
            .set(HardwareContext::new(&window).expect("should be able to create hardware context"))
            .expect("should only be called once");

        self.staging_belt
            .set(StagingBelt::new(1024))
            .expect("should only be called once");

        self.middleware
            .iter()
            .fold(window, |acc, m| m.configure_window(acc))
    }

    fn before_event_loop(
        &mut self,
        window: &winit::window::Window,
        event_loop_proxy: EventLoopProxy<EnginePacket>,
    ) {
        let hardware = self.hardware.get().expect("should exist just set");
        for middleware in self.middleware.iter_mut() {
            middleware.before_event_loop(window, event_loop_proxy.clone());
            middleware.on_hardware(hardware, window);
        }
    }

    fn before_event(
        &mut self,
        event: &winit::event::Event<EnginePacket>,
        context: &crate::desktop::DesktopContext,
    ) {
        for middleware in self.middleware.iter_mut() {
            middleware.before_event(event, context);
        }
    }

    fn on_window_event(
        &mut self,
        window_id: winit::window::WindowId,
        event: &winit::event::WindowEvent,
        context: &crate::desktop::DesktopContext,
    ) {
        match &event {
            winit::event::WindowEvent::Resized(ref size) => {
                if let Some(hardware) = self.hardware.get_mut() {
                    trace!("Resizing {:?}", size);
                    hardware.surface_config.height = size.height;
                    hardware.surface_config.width = size.width;
                    hardware
                        .surface
                        .configure(&hardware.device, &hardware.surface_config);
                }
            }
            _ => {}
        }

        for middleware in self.middleware.iter_mut() {
            middleware.on_window_event(window_id, event, context);
        }
    }

    fn on_window_redraw(
        &mut self,
        window_id: winit::window::WindowId,
        context: &crate::desktop::DesktopContext,
    ) {
        if let Some(hardware) = self.hardware.get_mut() {
            if let Ok(frame) = hardware.surface.get_current_texture() {
                // -- Configure redraw settings
                let texture_view_desc = self
                    .middleware
                    .iter_mut()
                    .fold(wgpu::TextureViewDescriptor::default(), |acc, m| {
                        m.configure_redraw_settings(acc)
                    });
                let view = &frame.texture.create_view(&texture_view_desc);

                // Initialize render setup types
                let staging_belt = self.staging_belt.get_mut().expect("should be enabled");
                let mut encoder: wgpu::CommandEncoder =
                    hardware
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("nebudeck-on-window-redraw"),
                        });

                // -- Before clear pass
                for middleware in self.middleware.iter_mut() {
                    middleware.on_window_redraw(window_id, &context);
                    middleware.before_clear_pass(staging_belt, view, &hardware);
                }

                {
                    // "Clear" render pass
                    let mut clear_rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clear_render_pass"),
                        // wgpu 0.18.0
                        // timestamp_writes: None,
                        // occlusion_query_set: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
                                    a: 1.0,
                                }),
                                // wgpu 0.18.0
                                // store: wgpu::StoreOp::Store,
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: {
                            // if self.app.enable_depth_stencil() {
                            //     Some(wgpu::RenderPassDepthStencilAttachment {
                            //         view: &self.depth_texture,
                            //         depth_ops: Some(wgpu::Operations {
                            //             load: wgpu::LoadOp::Clear(1.0),
                            //             store: true,
                            //         }),
                            //         stencil_ops: None,
                            //     })
                            // } else {
                            //     None
                            // }
                            None
                        },
                    });

                    // -- On clear pass
                    for middleware in self.middleware.iter_mut() {
                        middleware.on_clear_pass(staging_belt, &mut clear_rpass, view, &hardware);
                    }
                }

                // -- Before load pass
                for middleware in self.middleware.iter_mut() {
                    middleware.before_load_pass(staging_belt, view, &hardware);
                }

                {
                    // "Load" render pass
                    let mut load_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("load_render_pass"),
                        // wgpu 0.18.0
                        // timestamp_writes: None,
                        // occlusion_query_set: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                // wgpu 0.18.0
                                // store: wgpu::StoreOp::Store,
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: {
                            // if self.app.enable_depth_stencil() {
                            //     Some(wgpu::RenderPassDepthStencilAttachment {
                            //         view: &self.depth_texture,
                            //         depth_ops: Some(wgpu::Operations {
                            //             load: wgpu::LoadOp::Clear(1.0),
                            //             store: true,
                            //         }),
                            //         stencil_ops: None,
                            //     })
                            // } else {
                            //     None
                            // }
                            None
                        },
                    });

                    // -- On load pass
                    for middleware in self.middleware.iter_mut() {
                        middleware.on_load_pass(staging_belt, &mut load_pass, view, &hardware);
                    }
                }

                // -- Before frame present
                for middleware in self.middleware.iter_mut() {
                    middleware.before_frame_present(staging_belt, view, &hardware);
                }

                hardware.queue.submit(Some(encoder.finish()));
                staging_belt.finish();
                frame.present();
                staging_belt.recall();
            }
        }
    }

    fn on_new_events(
        &mut self,
        start_cause: winit::event::StartCause,
        context: &crate::desktop::DesktopContext,
    ) {
        for middleware in self.middleware.iter_mut() {
            middleware.on_new_events(start_cause, context);
        }
    }

    fn on_device_event(
        &mut self,
        window_id: winit::event::DeviceId,
        window_event: &winit::event::DeviceEvent,
        context: &crate::desktop::DesktopContext,
    ) {
        for middleware in self.middleware.iter_mut() {
            middleware.on_device_event(window_id, &window_event, context);
        }
    }

    fn on_user_event(&mut self, user: &EnginePacket, context: &crate::desktop::DesktopContext) {
        for middleware in self.middleware.iter_mut() {
            middleware.on_user_event(user, context);
        }
    }

    fn on_suspended(&mut self, context: &crate::desktop::DesktopContext) {
        for middleware in self.middleware.iter_mut() {
            middleware.on_suspended(context);
        }
    }

    fn on_resumed(&mut self, context: &crate::desktop::DesktopContext) {
        for middleware in self.middleware.iter_mut() {
            middleware.on_resumed(context);
        }
    }

    fn on_about_to_wait(&mut self, context: &crate::desktop::DesktopContext) {
        for middleware in self.middleware.iter_mut() {
            middleware.on_about_to_wait(context);
        }
    }

    fn on_loop_exiting(&mut self, context: &crate::desktop::DesktopContext) {
        for middleware in self.middleware.iter_mut() {
            middleware.on_loop_exiting(context);
        }
    }

    fn on_memory_warning(&mut self, context: &crate::desktop::DesktopContext) {
        for middleware in self.middleware.iter_mut() {
            middleware.on_memory_warning(context);
        }
    }

    fn after_event(&mut self, context: &crate::desktop::DesktopContext) {
        for middleware in self.middleware.iter_mut() {
            middleware.after_event(context);
        }
    }
}

/// Boxed render pipeline middleware,
///
pub type BoxedMiddleware= Pin<Box<dyn RenderPipelineMiddleware + Unpin>>;

/// Trait that enables access to the rendering pipeline,
///
#[allow(unused_variables)]
pub trait RenderPipelineMiddleware: DesktopApp {
    /// Converts the middleware into it's generic form,
    ///
    fn middleware(self) -> BoxedMiddleware
    where
        Self: Sized + Unpin + 'static,
    {
        Box::pin(self)
    }

    /// Configure various descriptors,
    ///
    fn configure_redraw_settings<'a: 'b, 'b>(
        &'a mut self,
        texture_view_desc: wgpu::TextureViewDescriptor<'b>,
    ) -> wgpu::TextureViewDescriptor<'b> {
        texture_view_desc
    }

    /// Called before event loop is starting and hardware context is created,
    fn on_hardware(&mut self, hardware: &HardwareContext, window: &winit::window::Window) {}

    /// Called before the clear render pass is created,
    ///
    fn before_clear_pass(
        &mut self,
        staging_belt: &mut StagingBelt,
        view: &TextureView,
        hardware: &HardwareContext,
    ) {
    }

    /// Called before the clear render pass is dropped,
    ///
    fn on_clear_pass<'a: 'b, 'b>(
        &'a mut self,
        staging_belt: &mut StagingBelt,
        rpass: &mut RenderPass<'b>,
        view: &TextureView,
        hardware: &HardwareContext,
    ) {
    }

    /// Called before the load render pass is created,
    ///
    fn before_load_pass(
        &mut self,
        staging_belt: &mut StagingBelt,
        view: &TextureView,
        hardware: &HardwareContext,
    ) {
    }

    /// Called before the load render pass is dropped,
    ///
    fn on_load_pass<'a: 'b, 'b>(
        &'a mut self,
        staging_belt: &mut StagingBelt,
        rpass: &mut RenderPass<'b>,
        view: &TextureView,
        hardware: &HardwareContext,
    ) {
    }

    fn before_frame_present(
        &mut self,
        staging_belt: &mut StagingBelt,
        view: &TextureView,
        hardware: &HardwareContext,
    ) {
    }
}

impl ControlBus for WgpuSystem {
    fn bind(&mut self, engine: EngineHandle) {
        for middleware in self.middleware.iter_mut() {
            middleware.bind(engine.clone());
        }
    }

    fn delegate(self, controller: impl Controller<Self>, engine: ForegroundEngine) -> BackgroundWork
    where
        Self: Sized,
    {
        self.engine
            .set(engine.engine_handle())
            .expect("should only be called once");

        controller.take_control(Box::new(self), engine)
    }
}

// TODO: finish porting from gamegamegame
// pub struct RenderContext {
//     /// Number of verticies
//     verticies: Option<u32>,
//     /// Number of indicies
//     indicies: Option<u32>,
//     /// Camera attached to this context
//     camera: Option<Camera>,
//     /// Camera uniform to use with the camera
//     camera_uniform: Option<CameraUniform>,
//     /// The range of instances to draw
//     draw_instances: Option<Range<u32>>,
//     /// Instance data to write to the instance buffer
//     instance_data: Option<Vec<InstanceRaw>>,
//     /// Executes the render against the render pass, in the context of the render pipeline
//     render: Option<Render>,
//     /// Compiles a .wgsl shader program
//     compile: Option<CompileShader>,
//     /// Lights uniform matrix
//     lights_uniform: Option<LightUniform>,
// }
