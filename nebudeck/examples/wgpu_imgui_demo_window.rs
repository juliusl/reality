use std::time::Instant;

use loopio::prelude::*;
use nebudeck::desktop::*;
use nebudeck::ext::imgui_ext::ImguiExt;
use nebudeck::ext::imgui_ext::ImguiMiddleware;
use nebudeck::ext::WgpuSystem;
use nebudeck::ext::*;
use nebudeck::ControlBus;

/// Demonstrates how to build on top of the WgpuSystem Desktop App implementation,
///
/// This examples how to customize a WgpuSystem w/ middleware using the ImguiMiddleware implementation.
///
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Next, create a workspace
    let mut workspace = CurrentDir.workspace();

    workspace.add_buffer(
        "test.md",
        r"
    ```runmd
    + .operation show_frame_editor                              # Shows frame editor
    <loopio.enable-wirebus>             a/demo.test2            # Enables the wire-bus for attribute at specific path

    <nebudeck.frame-editor>             a/demo.test2            # Enables the frame editor for an attribute at a specific path
    : .editor_name                      Demo editor
    : test .panel                       Test Panel              # Custom panels can be constructed from runmd
    : test .text-edit                   test_value              # Adds a text editor for the property test_value
    : test .text-display                test_value
    : test .text-edit                   test_not_str
    : test .text-display                test_not_str
    : test .usize-edit                  test_not_str
    : test .usize-display               test_not_str
    : test .action                      Run Test

    <a/demo.test2> hello world 2
    : .test_value   Test value
    : .test_not_str 10

    + .operation setup
    <demo.test> hello world
    ```
    ");

    // Build and compile an engine

    let (desktop, mut engine) = DevProject.new_project();
    engine.enable::<Test>();
    engine.enable::<Test2>();
    let engine = engine.compile(workspace).await;

    // Create the new WgpuSystem
    WgpuSystem::with(vec![ImguiMiddleware::new()
        .enable_imgui_demo_window()
        .enable_aux_demo_window()
        .middleware()])
    // Opens the window by passing control over to the desktop ControlBus
    .delegate(desktop, engine);

    Ok(())
}

#[derive(Reality, Debug, Clone, Default)]
#[reality(call = test_ui, plugin, group = "demo")]
struct Test {
    #[reality(derive_fromstr)]
    name: String,
    #[reality(wire)]
    value_str: String,
}

async fn test_ui(tc: &mut ThunkContext) -> anyhow::Result<()> {
    // Must cache before adding the node, otherwise the cache will not have the value
    tc.cache::<Test>().await;
    tc.find_and_cache::<EngineHandle>(true).await;
    tc.find_and_cache::<ParsedAttributes>(true).await;

    println!("Adding ui node {:?}", tc.attribute);
    tc.add_ui_node(|__tc, ui| {
        ui.window("test").build(|| {
            ui.text(format!("{:?}", Instant::now()));
            ui.text(format!("{:?}", __tc.attribute));
            if let Some(test) = __tc.cached::<Test>() {
                ui.text(test.name);

                if let Some(mut eh) = __tc.cached_mut::<EngineHandle>() {
                    ui.text("Operations:");
                    ui.popup("test_popup", || {
                        ui.text("finished");
                    });

                    for (idx, (op, __op)) in eh.operations.iter_mut().enumerate() {
                        ui.text(op);
                        if !__op.is_running() {
                            if ui.button(format!("start##{}", idx)) {
                                __op.spawn();
                            }
                        } else {
                            ui.text("Running");
                            if __op.is_finished() {
                                if let Ok(_) = __op.block_result() {
                                    ui.open_popup("test_popup");
                                }
                            }
                        }
                    }
                }

                if let Some(parsed) = __tc.cached::<ParsedAttributes>() {
                    ui.label_text(
                        "Number of parsed attributes",
                        parsed.attributes.len().to_string(),
                    );

                    let defined_properties = parsed
                        .properties
                        .defined
                        .iter()
                        .fold(0, |acc, d| acc + d.1.len());
                    ui.label_text(
                        "Number of properties defined",
                        defined_properties.to_string(),
                    );
                }
            } else {
                ui.text("Not found");
            }
        });
        false
    })
    .await;

    Ok(())
}

#[derive(Reality, Debug, Clone, Default)]
#[reality(call = test_2, plugin, group = "demo")]
struct Test2 {
    #[reality(derive_fromstr)]
    name: String,
    #[reality(wire)]
    test_value: String,
    #[reality(wire)]
    test_not_str: usize,
}

async fn test_2(tc: &mut ThunkContext) -> anyhow::Result<()> {
    println!("{:?}", tc.initialized::<Test2>().await);
    Ok(())
}
