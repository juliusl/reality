use std::fmt::Debug;
use std::sync::Arc;

use reality::prelude::*;

/// Type-alias for a frame middleware,
/// 
type FrameMiddlewareFn =
    Arc<dyn for<'a> Fn(&'a WireBus, &'a ThunkContext, Frame) -> Frame + Send + Sync + 'static>;

/// Trait for adding a wire protocol for a Reality type,
///
/// **Note** Based on `to_frames/from_frames` derivatives.
///
#[async_trait::async_trait]
pub trait WireExt {
    /// Extends the wire bus with a middleware fn assigned to the current node,
    ///
    async fn extend_wire_bus(
        &mut self,
        middleware: impl for<'a> Fn(&'a WireBus, &'a ThunkContext, Frame) -> Frame + Send + Sync + 'static,
    );
}

#[async_trait::async_trait]
impl WireExt for ThunkContext {
    async fn extend_wire_bus(
        &mut self,
        middleware: impl for<'a> Fn(&'a WireBus, &'a ThunkContext, Frame) -> Frame + Send + Sync + 'static,
    ) {
        unsafe {
            self.node_mut()
                .await
                .put_resource::<FrameMiddlewareFn>(Arc::new(middleware), self.attribute.map(|a| a.transmute()))
        }
    }
}

/// Converts the type being extended into wire format,
///
/// Middleware can be configured on the bus to operate on the frame before applying it.
///
#[derive(Default, Clone)]
pub struct WireBus {
    /// Current frame,
    ///
    pub frame: Frame,
}

impl Debug for WireBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WireBus").field("frame", &self.frame).finish()
    }
}

/// Plugin to enable the wire bus on an attribute,
/// 
#[derive(Reality, Default, Clone)]
#[reality(call=enable_wire_bus, plugin, rename = "enable-wirebus")]
pub struct EnableWireBus {
    /// Path to the attribute,
    /// 
    /// **Note**: A path must be assigned to an attribute in order for it to be 
    /// navigated to by this plugin.
    /// 
    #[reality(derive_fromstr)]
    path: String,
}

async fn enable_wire_bus(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<EnableWireBus>().await;

    if let Some(path) = tc.navigate(&init.path).await {
        eprintln!("Enabling frame {}", init.path);
        if let Some(enabled) = path.enable_frame().await? {
            let frame = enabled.initialized_frame().await;
    
            let wire_bus = WireBus { frame };
            unsafe {
                eprintln!("Putting wire bus \n{:#?}\n{:#?}", path.attribute, wire_bus);
                path.node_mut()
                    .await
                    .put_resource(wire_bus, path.attribute.map(|a| a.transmute()));
            };
        }
    }

    Ok(())
}
