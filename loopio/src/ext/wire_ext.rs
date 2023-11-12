use std::sync::Arc;

use reality::prelude::*;
use tracing::trace;

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
        middleware: impl Fn(&WireBus, &ThunkContext, Frame) -> Frame + Send + Sync + 'static,
    );
}

#[async_trait::async_trait]
impl WireExt for ThunkContext {
    async fn extend_wire_bus(
        &mut self,
        middleware: impl Fn(&WireBus, &ThunkContext, Frame) -> Frame + Send + Sync + 'static,
    ) {
        unsafe {
            self.node_mut()
                .await
                .put_resource(Box::new(middleware), self.attribute.map(|a| a.transmute()))
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
    frame: Frame,
    /// Middlware to run a frame through,
    ///
    middleware: Vec<MiddlewareFn>,
}

type MiddlewareFn =
    Arc<dyn Fn(&WireBus, &ThunkContext, Frame) -> Frame + Send + Sync + 'static>;

impl<B> SetupTransform<B> for WireBus
where
    B: ApplyFrame + Clone + ToFrame + Plugin,
{
    fn ident() -> &'static str {
        "wire"
    }

    fn setup_transform(
        resource_key: Option<&reality::ResourceKey<reality::Attribute>>,
    ) -> reality::Transform<Self, B> {
        let key = resource_key.copied();
        Self::default_setup(resource_key)
            .before_task(move |tc, mut bus, b| {
                trace!("Building new extension");
                let middleware = tokio::spawn(async move { tc.scan_node::<MiddlewareFn>().await });
                Box::pin(async move {
                    bus.middleware.extend(middleware.await?);
                    Ok((bus, b))
                })
            })
            .before_task(move |_, mut bus, b: anyhow::Result<B>| {
                trace!("Converting {} to Frame", std::any::type_name::<B>());
                Box::pin(async move {
                    if let Ok(b) = b {
                        let frame = b.clone().to_frame(key);
                        bus.frame = frame;
                        Ok((bus, Ok(b)))
                    } else {
                        Ok((bus, b))
                    }
                })
            })
            .user_task(|target, mut bus, b: anyhow::Result<B>| {
                trace!("Applying middleware to wire bus");
                Box::pin(async move {
                    let frame = { bus.frame.clone() };
                    let _bus = bus.clone();
                    let _target = target.clone();
                    bus.frame = {
                        bus.middleware
                            .iter()
                            .fold(frame, move |acc, m| m(&_bus, &_target, acc))
                    };
                   Ok((bus, b))
                })
            })
            .after(|_, bus, mut b: anyhow::Result<B>| {
                trace!("Applying frame back to {}", std::any::type_name::<B>());
                if let Ok(b) = b.as_mut() {
                    b.apply_frame(bus.frame.clone())?;
                }

                Ok((bus, b))
            })
    }
}

#[derive(Reality)]
#[reality(call=debug_wire, plugin, rename = "ext/wire.debug")]
pub struct DebugWire;

async fn debug_wire(tc: &mut ThunkContext) -> anyhow::Result<()> {
    tc.extend_wire_bus(|_, _, frame| {
        println!("{:#?}", frame);
        frame
    })
    .await;

    Ok(())
}
