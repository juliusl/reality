macro_rules! cfg_feature {
    ($ft:literal, {
        $($e:item)*
    }) => {
        $(
            #[cfg(feature = $ft)]
            $e
        )*
    };
}

cfg_feature!("desktop-imgui", {
    mod wgpu_ext;
    pub use wgpu_ext::BoxedMiddleware;
    pub use wgpu_ext::RenderPipelineMiddleware;
    pub use wgpu_ext::WgpuResourceManagementExt;
    pub use wgpu_ext::WgpuSystem;
});

#[cfg(feature = "desktop-imgui")]
pub mod imgui_ext;

pub mod aux_node;
pub use aux_node::*;

pub mod prelude {
    pub mod wgpu {
        #[cfg(feature = "desktop-imgui")]
        pub use wgpu_17::*;

        #[cfg(feature = "desktop-vnext")]
        pub use wgpu_18::*;
    }

    pub mod winit {
        #[cfg(feature = "desktop-vnext")]
        pub use winit_29::*;

        #[cfg(feature = "desktop-imgui")]
        pub use winit_27::*;
    }
}
