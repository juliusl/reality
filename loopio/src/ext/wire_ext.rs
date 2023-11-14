use std::fmt::Debug;

use reality::prelude::*;
use serde::Serialize;
use serde::Deserialize;
use tracing::info;

/// Converts the type being extended into wire format,
///
/// Middleware can be configured on the bus to operate on the frame before applying it.
///
#[derive(Default, Debug, Clone)]
pub struct WireBus {
    /// Current frame,
    ///
    pub frame: Frame,
}

/// Plugin to enable the wire bus on an attribute,
///
#[derive(Reality, Serialize, Deserialize, Default, Clone)]
#[reality(call=enable_wire_bus, plugin, rename = "enable-wirebus")]
pub struct EnableWireBus {
    /// Path to the attribute,
    ///
    /// **Note**: A path must be assigned to an attribute in order for it to be
    /// navigated to by this plugin.
    ///
    #[reality(derive_fromstr)]
    path: String,
    /// If true allows changes to be applied,
    ///
    allow_frame_updates: bool,
}

async fn enable_wire_bus(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<EnableWireBus>().await;

    if let Some(path) = tc.navigate(&init.path).await {
        info!("Enabling wire bus {}", init.path);
        if let Some(enabled) = path.enable_frame().await? {
            let frame = enabled.initialized_frame().await;

            let wire_bus = WireBus {
                frame,
            };
            unsafe {
                // Creates a new wire bus
                path.node_mut()
                    .await
                    .put_resource(wire_bus, path.attribute.map(|a| a.transmute()));

                // If enabled this will enable frame updates for the plugin,
                if init.allow_frame_updates {
                    path.node_mut().await.put_resource::<FrameUpdates>(
                        FrameUpdates::default(),
                        path.attribute.map(|a| a.transmute()),
                    );
                }
            };
        }
    }

    Ok(())
}
