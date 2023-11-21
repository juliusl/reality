use std::fmt::Debug;

use anyhow::anyhow;
use reality::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use tracing::info;

use crate::prelude::Action;

/// Converts the type being extended into wire format,
///
/// Middleware can be configured on the bus to operate on the frame before applying it.
///
#[derive(Default, Debug, Clone)]
pub struct WireBus {
    /// Current frame,
    ///
    frame: Frame,
}

impl WireBus {
    /// Returns a vector of packets currently stored on the bus,
    /// 
    pub fn packets(&self) -> Vec<FieldPacket> {
        // TODO: This could be optimized later, but for brevity this is what needs to be returned,
        [self.frame.recv.clone()]
            .iter()
            .chain(self.frame.fields.iter())
            .cloned()
            .collect::<Vec<FieldPacket>>()
    }
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

    if let Some(mut path) = tc.navigate(&init.path).await {
        info!("Enabling wire bus {}", init.path);
        if let Some(enabled) = path.context().enable_frame().await? {
            let attr = path.context().attribute.clone();
            let frame = enabled.initialized_frame().await;
            unsafe {
                // Creates a new wire bus
                path.context_mut().node_mut()
                    .await
                    .put_resource(WireBus { frame }, attr.transmute());

                // If enabled this will enable frame updates for the plugin,
                if init.allow_frame_updates {
                    path.context_mut().node_mut().await.maybe_put_resource::<FrameUpdates>(
                        FrameUpdates::default(),
                        attr.transmute(),
                    );
                }
            };
        }
        Ok(())
    } else {
        Err(anyhow!("Could not find resource {:?}", init.path))
    }
}
