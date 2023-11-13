use std::{collections::BTreeMap, ops::Deref};

use imgui::{CollapsingHeader, TreeNodeFlags};
use loopio::prelude::*;
use tracing::instrument;

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
                            field_map.insert(field_name.to_string(), wire_data.clone());

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
    field_map: &BTreeMap<String, Option<Vec<u8>>>,
    ui: &imgui::Ui,
    tc: &mut ThunkContext,
) {
    if let (Some(panel_name), Some(field)) = (panel_name, field) {
        if panel_name == name {
            if let Some(val) = field_map
                .get(field)
                .and_then(|f| f.clone())
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
                    }
                }
            }
        }
    }
}

fn text_display(
    panel_name: Option<&String>,
    field: Option<&String>,
    name: &String,
    field_map: &BTreeMap<String, Option<Vec<u8>>>,
    ui: &imgui::Ui,
) {
    if let (Some(panel_name), Some(field)) = (panel_name, field) {
        if panel_name == name {
            if let Some(val) = field_map
                .get(field)
                .and_then(|f| f.clone())
                .and_then(|v| bincode::deserialize::<String>(&v).ok())
            {
                ui.label_text(field, val.as_str());
            }
        }
    }
}
