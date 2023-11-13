use std::{collections::BTreeMap, ops::Deref};

use imgui::TreeNodeFlags;
use loopio::prelude::*;

use crate::ext::imgui_ext::ImguiExt;

/// Widget to edit frames of an attribute,
///
#[derive(Reality, Default, Clone)]
#[reality(call = enable_frame_editor, plugin, rename = "frame-editor")]
pub struct FrameEditor {
    /// Path to the attribute being edited,
    ///
    #[reality(derive_fromstr)]
    path: String,
    /// Name of the editor,
    ///
    #[reality(option_of=String)]
    editor_name: Option<String>,
    /// Map of panels,
    ///
    #[reality(map_of=String)]
    panel: BTreeMap<String, String>,
    /// Text edit inputs,
    ///
    #[reality(vec_of=Tagged<String>, rename="text-edit")]
    text_edit: Vec<Tagged<String>>,
    /// Text edit inputs,
    ///
    #[reality(vec_of=Tagged<String>, rename="text-display")]
    text_display: Vec<Tagged<String>>,
    /// usize edit inputs,
    ///
    #[reality(vec_of=Tagged<String>, rename="usize-edit")]
    usize_edit: Vec<Tagged<String>>,
    /// usize edit inputs,
    ///
    #[reality(vec_of=Tagged<String>, rename="usize-display")]
    usize_display: Vec<Tagged<String>>,
}

async fn enable_frame_editor(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<FrameEditor>().await;

    eprintln!("Enabling frame editor -- {}", init.path);
    if let Some(mut editing) = tc.navigate(&init.path).await {
        eprintln!("Found path -- {:?}", editing.attribute);
        {
            let node = editing.node().await;
            if let Some(bus) =
                node.current_resource::<WireBus>(editing.attribute.map(|a| a.transmute()))
            {
                eprintln!("Found wire bus");
                drop(node);
                editing.write_cache(bus);
            }
        }

        editing
            .add_ui_node(move |tc, ui| {
                let init = init.clone();

                let title = init.editor_name.unwrap_or(format!("{:?}", tc.attribute));

                ui.window(format!("Frame Editor - {}", title)).build(|| {
                    if let Some(rk) = tc.attribute.as_ref().map(|r| r.key()) {
                        ui.label_text("Resource Key", rk.to_string());
                    }

                    let mut field_map = BTreeMap::new();
                    
                    if let Some(wb) = tc.cached_ref::<WireBus>() {
                        let mut render = vec![];

                        for (idx, packet) in wb.frame.iter().enumerate() {
                            let FieldPacket {
                                data_type_name,
                                data_type_size,
                                field_offset,
                                owner_name,
                                field_name,
                                attribute_hash,
                                data,
                                wire_data,
                            } = packet;
                            field_map.insert(field_name.to_string(), (data_type_name.clone(), wire_data.clone()));

                            render.push(move || {
                                ui.text(format!("Field Packet {idx}:"));
                                ui.label_text("field_name", field_name);
                                ui.label_text("type_name", data_type_name);
                                ui.label_text("type_size", data_type_size.to_string());
                                ui.label_text("field_offset", field_offset.to_string());
                                ui.label_text("owner_name", owner_name);
                                ui.label_text("attribute hash", format!("{:?}", attribute_hash));

                                if let Some(_) = data {
                                    ui.text("Has data");
                                }

                                if let Some(bin) = wire_data {
                                    ui.text("Has binary data");
                                    if ui.button("Deserialize") {
                                        if let Ok(s) = bincode::deserialize::<String>(&bin) {
                                            println!("Deserialized: {s}");
                                        }
                                    }
                                }

                                // TODO -- There's a lot of tools that can be added.
                            });
                        }

                        if ui.collapsing_header("Wire Bus", TreeNodeFlags::empty()) {
                            for r in render.drain(..) {
                                r();
                            }
                        }
                    }

                    for (name, title) in init.panel.iter() {
                        if ui.collapsing_header(title, TreeNodeFlags::DEFAULT_OPEN) {
                            // This can be optimized later -
                            for (panel_name, field) in
                                init.text_edit.iter().map(|t| (t.tag(), t.value()))
                            {
                                text_edit(panel_name, field, name, &field_map, ui, tc);
                            }

                            for (panel_name, field) in
                                init.text_display.iter().map(|t| (t.tag(), t.value()))
                            {
                                text_display(panel_name, field, name, &field_map, ui)
                            }

                            for (panel_name, field) in
                                init.usize_edit.iter().map(|t| (t.tag(), t.value()))
                            {
                                usize_edit(panel_name, field, name, &field_map, ui, tc);
                            }

                            for (panel_name, field) in
                                init.usize_display.iter().map(|t| (t.tag(), t.value()))
                            {
                                usize_display(panel_name, field, name, &field_map, ui)
                            }
                        }
                    }
                });
                true
            })
            .await;
    }

    Ok(())
}

/// Displays a text edit field,
///
fn text_edit(
    panel_name: Option<&String>,
    field: Option<&String>,
    name: &String,
    field_map: &BTreeMap<String, (String, Option<Vec<u8>>)>,
    ui: &imgui::Ui,
    tc: &mut ThunkContext,
) {
    if let (Some(panel_name), Some(field)) = (panel_name, field) {
        if panel_name == name {
            if let Some(val) = field_map
                .get(field)
                .filter(|(f, _)| {
                    f == std::any::type_name::<String>()
                })
                .and_then(|f| f.1.clone())
                .and_then(|v| bincode::deserialize::<String>(&v).ok())
            {
                ui.text(format!("Current Value -- {val}"));

                if !tc.kv_contains::<String>(field) {
                    tc.store_kv(field, val.to_string());
                }

                if let Some((key, mut value)) = tc.fetch_mut_kv::<String>(field) {
                    ui.input_text(format!("{}##{:?}", field, key), &mut value)
                        .build();

                    if value.deref() != &val {
                        ui.text("Value has changed");
                        ui.same_line();
                        let token = ui.push_id(key.key().to_string());
                        if ui.button("Save change") {
                            eprintln!("TODO: Actually change the value");
                        }
                        token.end();
                    }
                }
            } else {
                ui.text("Not a String");
            }
        }
    }
}

fn text_display(
    panel_name: Option<&String>,
    field: Option<&String>,
    name: &String,
    field_map: &BTreeMap<String, (String, Option<Vec<u8>>)>,
    ui: &imgui::Ui,
) {
    if let (Some(panel_name), Some(field)) = (panel_name, field) {
        if panel_name == name {
            if let Some(val) = field_map
                .get(field)
                .filter(|(f, _)| {
                    f == std::any::type_name::<String>()
                })
                .and_then(|f| f.1.clone())
                .and_then(|v| bincode::deserialize::<String>(&v).ok())
            {
                ui.label_text(field, val.as_str());
            } else {
                ui.text("Not a String");
            }
        }
    }
}

/// Displays a text edit field,
///
fn usize_edit(
    panel_name: Option<&String>,
    field: Option<&String>,
    name: &String,
    field_map: &BTreeMap<String, (String, Option<Vec<u8>>)>,
    ui: &imgui::Ui,
    tc: &mut ThunkContext,
) {
    if let (Some(panel_name), Some(field)) = (panel_name, field) {
        if panel_name == name {
            if let Some(val) = field_map
                .get(field)
                .filter(|(f, _)| {
                    f == std::any::type_name::<usize>()
                })
                .and_then(|f| f.1.clone())
                .and_then(|v| bincode::deserialize::<i32>(&v).ok())
            {
                ui.text(format!("Current Value -- {val}"));

                if !tc.kv_contains::<i32>(field) {
                    tc.store_kv(field, val);
                }

                if let Some((key, mut value)) = tc.fetch_mut_kv::<i32>(field) {
                    ui.input_int(format!("{}##{:?}", field, key), &mut value)
                        .build();

                    if value.deref() != &val {
                        ui.text("Value has changed");
                    }
                }
            } else {
                ui.text("Not usize");
            }
        }
    }
}

fn usize_display(
    panel_name: Option<&String>,
    field: Option<&String>,
    name: &String,
    field_map: &BTreeMap<String, (String, Option<Vec<u8>>)>,
    ui: &imgui::Ui,
) {
    if let (Some(panel_name), Some(field)) = (panel_name, field) {
        if panel_name == name {
            if let Some(val) = field_map
                .get(field)
                .filter(|(f, _)| {
                    f == std::any::type_name::<usize>()
                })
                .and_then(|f| f.1.clone())
                .and_then(|v| bincode::deserialize::<i32>(&v).ok())
            {
                ui.label_text(field, val.to_string());
            } else {
                ui.text("Not usize");
            }
        }
    }
}