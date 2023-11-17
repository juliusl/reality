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
    + .operation show_frame_editor                                              # Shows frame editor
    <loopio.enable-wirebus>                 demo://call_test_2/a/demo.test2     # Enables the wire-bus for attribute at specific path
    : .allow_frame_updates                  true
    
    <loopio.enable-wirebus>                 b/nebudeck.frame-editor             # Enables the wire-bus for attribute at specific path
    
    <nebudeck.frame-editor>                 b/nebudeck.frame-editor             # Enables the frame editor for the frame editor
    |# title = Demo editor Demo editor 2

    # -- # Demo: Customizable editor for editing and launching plugins
    # -- Also demonstrates the additional markup support
    <b/nebudeck.frame-editor>               demo://call_test_2/a/demo.test2     # Enables the frame editor for an attribute at a specific path
    |# title = Demo editor for test2

    # -- Example: Panel of customizable widgets
    # -- This example shows how to configure a customizable panel
    :  test         .panel                  Test Panel                          # Custom panels can be constructed from runmd
    |# help         = This is an example help documentation
    
    # -- Example: Configuring a property edit widget for a property
    # -- This is an example of editing a text value
    : test          .edit                   test_value
    |# title        = Test edit
    |# widget       = text
    |# help         = This is some example help documentation.
    
    # -- Example: Runs the plugin w/ the edited settings
    : test          .action             Run Test
    |# title        = Run Test
    |# description  = Runs a test

    + .operation    call_test_2
    <a/demo.test2> hello world 2        # Test comment a
    : .test_value   Test value          # Test comment b
    : .test_not_str 10                  # Test comment c

    + .operation setup
    <demo.test> hello world

    + .sequence start_demo
    : .once show_frame_editor
    : .loop false

    + .host demo
    : .start    start_demo
    : .action   call_test_2/a/demo.test2
    |# help = Indexes a path to a plugin

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
    let init = Interactive.create::<Test2>(tc).await;
    println!("{:#?}", init);
    Ok(())
}
