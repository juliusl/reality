use loopio::prelude::*;
use nebudeck::desktop::*;
use nebudeck::ext::WgpuSystem;
use nebudeck::ControlBus;
use nebudeck::ext::imgui_ext::ImguiMiddleware;
use nebudeck::ext::*;

/// Demonstrates how to build on top of the WgpuSystem Desktop App implementation,
/// 
/// This examples how to customize a WgpuSystem w/ middleware using the ImguiMiddleware implementation.
/// 
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // First create a controller, in this case the Desktop controller is required
    let desktop = Desktop::<()>::new()?;

    // Next, create a workspace
    let workspace = CurrentDir.workspace();

    // Build and compile an engine
    let engine = Engine::builder()
        .build()
        .compile(workspace)
        .await;
    
    // Create the new WgpuSystem
    WgpuSystem::with(vec![
        ImguiMiddleware::new().enable_demo_window().middleware()
    ])
    // Opens the window by passing control over to the desktop ControlBus
    .delegate(desktop, engine);

    Ok(())
}
