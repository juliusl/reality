use loopio::prelude::StorageTarget;
use loopio::prelude::ThunkContext;
use wgpu::BindGroup;
use wgpu::Buffer;
use wgpu::RenderPipeline;
use wgpu::IndexFormat;

/// Adds extensions to ThunkContext for handling wgpu primitives as resources w/ during 
/// thunk execution.
/// 
macro_rules! define_wgpu_resource_management {
    (
        $(#[$meta:meta])*
        pub trait $trait:ident { 
            [$(
                $(#[doc = $doc:literal])*
                ($set:ident, $take:ident, $ty:ty)
            ),*
            ]
        }
    ) => {
        $(#[$meta])*
        pub trait $trait {
            $(
                $(#[doc = $doc])*
                async fn $set(&mut self, resource: $ty);
                $(#[doc = $doc])*
                async fn $take(&mut self) -> Option<$ty>;
            )*
        }

        $(#[$meta])*
        impl $trait for ThunkContext {
            $(
                async fn $set(&mut self, instances: $ty) {
                    let mut transport = self.write_transport().await;

                    transport.put_resource(instances, None);
                }

                async fn $take(&mut self) -> Option<$ty> {
                    let mut transport = self.write_transport().await;

                    return transport.take_resource(None).map(|r| *r);
                }
            )*
        }
    };
}

define_wgpu_resource_management! {
    #[async_trait::async_trait]
    pub trait WgpuResourceManagementExt {
        [
            /// Handle setting/taking wgpu RenderPipeline
            (set_render_pipeline, take_render_pipeline, RenderPipeline),
            /// Handle setting/taking wgpu lighting RenderPipeline
            (set_lighting_pipeline, take_lighting_pipeline, RenderPipeline),
            /// Handle settings/taking render pass index buffer,
            (set_index_buffer, take_index_buffer, Buffer),
            /// Handle setting/taking render pass instance buffer,
            (set_instance_buffer, take_instance_buffer, Buffer),
            /// Handle setting/taking render pass vertex buffer,
            (set_vertex_buffer, take_vertex_buffer, Buffer),
            /// Handle setting/taking render pass camera buffer,
            (set_camera_buffer, take_camera_buffer, Buffer),
            /// Handle setting/taking render pass camera buffer,
            (set_lighting_buffer, take_lighting_buffer, Buffer),
            /// Handles setting/taking render pass main bind group,
            (set_bind_group, take_bind_group, BindGroup),
            /// Handles setting/taking render pass camera bind group,
            (set_camera_bind_group, take_camera_bind_group, BindGroup),
            /// Handles setting/taking render pass camera bind group,
            (set_lighting_bind_group, take_lighting_bind_group, BindGroup),
            /// Handles setting/taking the render passes index format,
            (set_index_format, take_index_format, IndexFormat)
        ]
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
