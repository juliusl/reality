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
                    let mut transport = self.write_transport().await;

                    transport.put_resource(instances, None);
                }

                async fn [<take_ $suffix>](&mut self) -> Option<$ty> {
                    let mut transport = self.write_transport().await;

                    return transport.take_resource(None).map(|r| *r);
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
