use std::time::Instant;

use loopio::foreground::ForegroundEngine;
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
fn main() -> anyhow::Result<()> {
    // Build and compile an engine
    let (desktop, mut engine) = DevProject.open_project(EmptyWorkspace.workspace());
    engine.workspace_mut().add_buffer(
        "test.md",
        r"
    ```runmd
    # -- # Enable the wirebus on the demo frame editor
    + .operation debug_show_frame_editor
    <loopio.enable-wirebus>                 demo://show_frame_editor/b/nebudeck.frame-editor
    
    # -- Debug the frame editor w/ a frame editor
    <nebudeck.frame-editor>                 demo://show_frame_editor/b/nebudeck.frame-editor
    |# title = Demo editor Demo editor 2
    
    + .operation show_frame_editor
    |# help = Shows a frame editor example

    <loopio.enable-wirebus>                 demo://call_test_2/a/demo.test2
    : .allow_frame_updates                  true

    # -- # Demo: Customizable editor for editing and launching plugins
    # -- Also demonstrates the additional markup support

    <b/nebudeck.frame-editor>               demo://call_test_2/a/demo.test2
    |# title = Demo editor for test2

    # -- Example: Panel of customizable widgets
    # -- This example shows how to configure a customizable panel
    
    :  test         .panel                  Test Panel
    |# help         = This is an example help documentation
    
    # -- Example: Configuring a property edit widget for a property
    # -- This is an example of editing a text value
    : test          .edit                   test_value
    |# title        = Test edit
    |# widget       = input_text
    |# help         = This is some example help documentation.
    
    # -- Example: Runs the plugin w/ the edited settings
    : test          .action             Run Test
    |# title        = Run Test
    |# description  = Runs a test

    + .operation    call_test_2
    # -- # Example User Plugin
    # -- Simple plugin that prints out debug info
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

    # -- # Example of a host action title
    : .action   call_test_2/a/demo.test2
    |# help = Indexes a path to a plugin

    # -- # Example of an action to show a frame editor
    : .action   show_frame_editor/b/nebudeck.frame-editor
    ```
    ");
    engine.enable::<Test>();
    engine.enable::<Test2>();

    let foreground = ForegroundEngine::new(engine);

    // Create the new WgpuSystem
    WgpuSystem::with(vec![ImguiMiddleware::new()
        .enable_imgui_demo_window()
        .enable_aux_demo_window()
        .middleware()])
    // Opens the window by passing control over to the desktop ControlBus
    .delegate(desktop, foreground);

    Ok(())
}

#[derive(Reality, Debug, Clone, Default)]
#[reality(call = test_ui, plugin, group = "demo")]
struct Test {
    #[reality(derive_fromstr)]
    name: String,
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

                if let Some(_eh) = __tc.cached_mut::<EngineHandle>() {
                    ui.text("Operations:");
                    ui.popup("test_popup", || {
                        ui.text("finished");
                    });

                    // for (idx, (op, __op)) in eh.operations.iter_mut().enumerate() {
                    //     ui.text(op);
                    //     if !__op.is_running() {
                    //         if ui.button(format!("start##{}", idx)) {
                    //             __op.spawn();
                    //         }
                    //     } else {
                    //         ui.text("Running");
                    //         if __op.is_finished() {
                    //             if let Ok(_) = __op.block_result() {
                    //                 ui.open_popup("test_popup");
                    //             }
                    //         }
                    //     }
                    // }
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
    test_value: String,
    test_not_str: usize,
}

async fn test_2(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = Remote.create::<Test2>(tc).await;
    println!("{:#?}", init);
    tc.print_debug_info();

    Ok(())
}

