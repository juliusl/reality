use imgui::InputTextCallbackHandler;
use imgui::TreeNodeFlags;
use loopio::action::LocalAction;
use loopio::action::TryCallExt;
use loopio::foreground::ForegroundEngine;
use loopio::prelude::AttributeType;
use loopio::prelude::*;
use nebudeck::desktop::*;
use nebudeck::ext::imgui_ext::ImguiExt;
use nebudeck::ext::imgui_ext::ImguiMiddleware;
use nebudeck::ext::imgui_ext::UiNode;
use nebudeck::ext::WgpuSystem;
use nebudeck::ext::*;
use nebudeck::widgets::UiDisplayMut;
use nebudeck::widgets::UiFormatter;
use nebudeck::ControlBus;
use std::time::Instant;

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
    <demo.processwizard>      cargo
    |# address = test://process_wizard
    : .arg --help

    + .sequence start_demo
    : .once show_frame_editor
    : .loop false

    + .host demo
    : .start    start_demo

    : .action   demo_proc/democmd/loopio.std.process

    # -- # Example of a host action title
    : .action   call_test_2/a/demo.test2
    |# help = Indexes a path to a plugin

    # -- # Example of an action to show a frame editor
    : .action   show_frame_editor/b/nebudeck.frame-editor
    ```
    ",
    );
    engine.enable::<Test>();
    engine.enable_as::<ProcessWizard, Process>();

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
#[reality(call = test, plugin, group = "demo")]
struct Test {
    #[reality(derive_fromstr)]
    name: String,
    test_value: String,
    test_not_str: usize,
}

async fn test(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = Local.create::<Test>(tc).await;
    println!("{:#?}", init);
    tc.print_debug_info();

    let mut storage = tc.transient.storage.write().await;
    init.pack::<Shared>(&mut storage);

    Ok(())
}

#[derive(Reality, Debug, Default, Clone)]
#[reality(call = process_wizard, replace=Process, plugin, group = "demo")]
struct ProcessWizard;

impl ProcessWizard {
    async fn edit_program_name(mut tc: ThunkContext) -> anyhow::Result<ThunkContext> {
        let process = Remote.create::<Process>(&mut tc).await;

        let wire_server = WireServer::<Process>::new(&mut tc).await?;

        tc.write_cache(VirtualProcess::new(process.clone()));
        tc.write_cache(process);
        tc.write_cache(wire_server);

        tc.push_ui_node(|ui| {
            if let Err(err) = ProcessWizard.fmt(ui) {
                ui.imgui.text(format!("{err}"));
            }
            true
        });

        Ok(tc)
    }
}

async fn process_wizard(tc: &mut ThunkContext) -> anyhow::Result<()> {
    if let Some(eh) = tc.engine_handle().await {
        // Build a local action
        // **Note** This could be a remote action but since there is no state there's no
        // point in initializing as a RemoteAction.
        let init = LocalAction.build::<Process>(tc).await;
        eprintln!("{:?}", tc.decoration);

        // Bind a task that defines the UI node and dependencies
        let init = init.bind_task("edit_program_name", ProcessWizard::edit_program_name);

        // Publish the remote action as a hosted resource
        let mut _a = init.publish(eh.clone()).await?;

        // Get the hosted resource published from the action
        let mut _a = eh.hosted_resource(_a.to_string()).await?;

        // Call a task on the hosted resource that will build the ui node
        if let Some(_tc) = _a.try_call("edit_program_name").await? {
            if let Some(mut nodes) = _tc
                .transient
                .storage
                .write()
                .await
                .take_resource::<Vec<UiNode>>(_tc.attribute.transmute())
            {
                // TODO -- This needs to be added back to the core library
                for n in nodes.iter_mut() {
                    n.context.decoration = tc.decoration.clone();
                }
                // Transfer transient storage resources over to the current context
                tc.transient
                    .storage
                    .write()
                    .await
                    .put_resource(*nodes, ResourceKey::root());
            }
        }
    }

    Ok(())
}

impl UiDisplayMut for ProcessWizard {
    fn fmt(&mut self, __ui: &UiFormatter<'_>) -> anyhow::Result<()> {
        // TODO -- improvements,
        //  Builder from ui formatter?
        // - .section(|ui| { }) => tc.maybe_write_kv(String, Vec<fn(&mut UiFormatter<'_>)>>)
        __ui.push_section("tools", |ui| {
            // Prepare current frame updates
            let mut pending_changes = vec![];

            // TODO: Currently this is the most straightforward way of applying this pattern
            if let Some(mut cached) = ui
                .tc
                .lock()
                .unwrap()
                .get_mut()
                .unwrap()
                .cached_mut::<VirtualProcess>()
            {
                // cached["program"].clone().start_tx()
                let tx = cached.program.clone().start_tx();

                if let Ok(next) = tx
                    .next(|mut n| {
                        n.fmt(ui)?;
                        Ok(n)
                    })
                    .finish()
                {
                    next.view_value(|r| {
                        eprintln!("Change -- {:?} {r}", Instant::now());
                        cached.program.pending();
                    });

                    if cached.program.is_pending() {
                        let packet = cached.program.encode();
                        pending_changes.push(("process_wizard", packet));
                    }
                }
            }

            for (label, packet) in pending_changes.drain(..) {
                ui.push_pending_change(label, packet);
            }
        });

        __ui.show_section("tools", |title, ui, mut inner| {
            let imgui = &ui.imgui;

            imgui
                .window(title)
                .size([600.0, 800.0], imgui::Condition::Appearing)
                .build(move || {
                    inner.fmt(ui).unwrap();

                    let mut current_frame_updates = FrameUpdates::default();

                    let pending_changes = ui.for_each_pending_change(|name, fp| {
                        if ui
                            .imgui
                            .collapsing_header(format!("DEBUG: {name}"), TreeNodeFlags::empty())
                        {
                            imgui.text(format!("{:#?}", fp));
                        }
                        current_frame_updates.frame.fields.push(fp.clone());
                    });

                    if pending_changes >= 1 {
                        ui.frame_updates.replace(current_frame_updates);
                    }

                    ui.show_call_button();

                    imgui.label_text("number of pending changes", pending_changes.to_string());
                });
        });

        Ok(())
    }
}

#[derive(Reality, Default, Clone)]
#[reality(replace = VirtualProcess, call = init_command, plugin)]
pub struct UiProcess;

async fn init_command(_tc: &mut ThunkContext) -> anyhow::Result<()> {
    Ok(())
}

struct _InputText;

impl InputTextCallbackHandler for _InputText {
    fn char_filter(&mut self, c: char) -> Option<char> {
        Some(c)
    }

    fn on_completion(&mut self, _: imgui::TextCallbackData) {}

    fn on_edit(&mut self, _: imgui::TextCallbackData) {}

    fn on_history(&mut self, _: imgui::HistoryDirection, _: imgui::TextCallbackData) {}

    fn on_always(&mut self, _: imgui::TextCallbackData) {}
}
