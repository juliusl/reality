use std::cell::RefCell;
use std::future::IntoFuture;
use std::sync::OnceLock;
use std::time::Instant;

use anyhow::anyhow;
use imgui::InputTextCallbackHandler;
use imgui::Ui;
use loopio::action::LocalAction;
use loopio::action::RemoteAction;
use loopio::action::TryCallExt;
use loopio::foreground::ForegroundEngine;
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
    engine.enable::<ProcessWizard>();

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
    let init = Remote.create::<Test>(tc).await;
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

        tc.write_cache(VirtualProcess::new(process.clone()));
        tc.write_cache(process);

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
        // Build a remote action
        let init = RemoteAction.build::<Process>(tc).await;
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
            let mut current_frame_updates = FrameUpdates::default();
            ui.for_each_pending_change(|_, p| {
                current_frame_updates.0.fields.push(p.clone());
            });

            let mut pending_changes = vec![];
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
                    // TODO -- ui.push_confirmation(|ui|{ })
                }

                if let Ok(deco) = ui.decorations.read() {
                    ui.imgui.text(format!("{:#?}", deco));

                    if let Some(address) = deco
                        .get()
                        .and_then(|d| d.comment_properties.as_ref())
                        .and_then(|d| d.get("address"))
                    {
                        if let Some(bg) = ui.eh.lock().unwrap().background() {
                            if let Ok(mut call) = bg.call(address) {
                                match call.status() {
                                    loopio::background_work::CallStatus::Enabled => {
                                        if ui.imgui.button("Run") {
                                            call.spawn_with_updates(current_frame_updates);
                                        }
                                    },
                                    loopio::background_work::CallStatus::Disabled => {},
                                    loopio::background_work::CallStatus::Running => {
                                        ui.imgui.text("Running");

                                        ui.imgui.same_line();
                                        if ui.imgui.button("Cancel") {
                                            call.cancel();
                                        }
                                    },
                                    loopio::background_work::CallStatus::Pending => {
                                        let _ = call.into_foreground().unwrap();
                                        eprintln!(
                                            "Background work finished"
                                        );
                                    },
                                }
                            }
                        }
                    }
                }
            }

            for (label, packet) in pending_changes.drain(..) {
                ui.push_pending_change(label, packet);
            }
        });

        __ui.show_section("tools", |ui, mut inner| {
            let imgui = &ui.imgui;

            imgui
                .window("tools")
                .size([600.0, 800.0], imgui::Condition::Appearing)
                .build(move || {
                    inner.fmt(ui).unwrap();

                    let pending_changes = ui.for_each_pending_change(|name, fp| {
                        imgui.text(format!("{name}:\n{:#?}", fp));
                    });

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
