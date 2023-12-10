use anyhow::anyhow;
use loopio::action::RemoteAction;
use loopio::prelude::*;
use serde::Serialize;

use imgui::Ui;

use crate::ext::imgui_ext::AuxUiNode;
use crate::ext::imgui_ext::UiNode;
use crate::ext::imgui_ext::UiTypeNode;

use super::UiDisplayMut;

pub struct Input<'a> {
    /// Frame that is being edited,
    ///
    packet: &'a mut FieldPacket,
}

impl<'a> Input<'a> {
    /// Edit a packet,
    ///
    pub fn edit<'b>(packet: &'a mut FieldPacket) -> Input<'b>
    where
        'a: 'b,
    {
        Self { packet }
    }
}

impl Input<'_> {
    /// Shows an input widget for a field packet,
    ///
    pub fn show(&mut self, ui: &Ui) -> Option<FieldOwned<FieldPacket>> {
        let label = self.packet.field_name.as_str();

        let mut changes = vec![];

        fn push_change<T>(
            data: &[u8],
            value: &T,
            new_packet: impl FnOnce() -> FieldPacket,
            changes: &mut Vec<FieldOwned<FieldPacket>>,
        ) where
            T: ?Sized + Serialize,
        {
            if let Ok(_b) = bincode::serialize(value) {
                if &_b != data {
                    let mut edited = new_packet();
                    edited.wire_data = Some(_b);
                    changes.push(edited.into_field_owned());
                }
            }
        }

        if let Some(data) = self.packet.wire_data.as_ref() {
            // if let Ok(mut f_value) = bincode::deserialize(&data) {
            //     ui.input_float(label, &mut f_value).build();

            //     push_change(&data, &f_value, || self.packet.clone(), &mut changes);
            // }

            // if let Ok(mut f_2_value) = bincode::deserialize::<[f32; 2]>(&data) {
            //     ui.input_float2(label, &mut f_2_value).build();

            //     push_change(&data, &f_2_value, || self.packet.clone(), &mut changes);
            // }

            // if let Ok(mut f_3_value) = bincode::deserialize::<[f32; 3]>(&data) {
            //     ui.input_float3(label, &mut f_3_value).build();

            //     push_change(&data, &f_3_value, || self.packet.clone(), &mut changes);
            // }

            // if let Ok(mut f_4_value) = bincode::deserialize::<[f32; 4]>(&data) {
            //     ui.input_float4(label, &mut f_4_value).build();

            //     push_change(&data, &f_4_value, || self.packet.clone(), &mut changes);
            // }

            // if let Ok(mut i_value) = bincode::deserialize::<i32>(&data) {
            //     ui.input_int(label, &mut i_value).build();

            //     push_change(&data, &i_value, || self.packet.clone(), &mut changes);
            // }

            // if let Ok(mut i_2_value) = bincode::deserialize::<[i32; 2]>(&data) {
            //     ui.input_int2(label, &mut i_2_value).build();

            //     push_change(&data, &i_2_value, || self.packet.clone(), &mut changes);
            // }

            // if let Ok(mut i_3_value) = bincode::deserialize::<[i32; 3]>(&data) {
            //     ui.input_int3(label, &mut i_3_value).build();

            //     push_change(&data, &i_3_value, || self.packet.clone(), &mut changes);
            // }

            // if let Ok(mut i_4_value) = bincode::deserialize::<[i32; 4]>(&data) {
            //     ui.input_int4(label, &mut i_4_value).build();

            //     push_change(&data, &i_4_value, || self.packet.clone(), &mut changes);
            // }

            if let Ok(mut text) = bincode::deserialize::<String>(&data) {
                ui.input_text(label, &mut text).build();

                push_change(&data, &text, || self.packet.clone(), &mut changes);
            }

            // if let Ok(mut text) = bincode::deserialize::<String>(&data) {
            //     ui.input_text_multiline(label, &mut text, ui.content_region_avail())
            //         .build();

            //     push_change(&data, &text, || self.packet.clone(), &mut changes);
            // }

            // if let Some(_c) = ui.begin_combo(label, "") {}
            // if let Some(_w) = ui.window(label).begin() {}
            // if let Some(_c) = ui.child_window(label).begin() {}
        }

        changes.pop()

        // ui.input_scalar(label, value);
        // ui.input_scalar_n(label, value);

        // if let Some(field) = widget.field_map.get(&widget.field) {
        //     let mut changes = vec![];

        //     // Iterate through each discovered packet,
        //     for p in field.iter() {
        //         if let Some((f, mut field)) =
        //             tc.fetch_mut_kv::<(String, String, FieldPacket)>(&p.field_name)
        //         {
        //             // Display any doc headers before this
        //             for d in &widget.doc_headers {
        //                 ui.text(d);
        //             }

        //             // Enables text editing input
        //             ui.input_text(&widget.title, &mut field.1).build();

        //             // Modification actions
        //             if field.0 != field.1 {
        //                 ui.text("Value has changed");
        //                 ui.same_line();
        //                 if ui.button("Reset") {
        //                     field.1 = field.0.to_string();
        //                 }

        //                 ui.same_line();
        //                 if ui.button("Save Changes") {
        //                     field.0 = field.1.to_string();
        //                     if let (mut field, Ok(wire)) =
        //                         (field.2.clone(), bincode::serialize(&field.0.deref()))
        //                     {
        //                         field.wire_data = Some(wire);
        //                         changes.push(field);
        //                     }
        //                 }
        //             }

        //             // Show the help popup
        //             let key = format!("help_popup##{}", f.key());
        //             if let Some(help) = widget.help.as_ref() {
        //                 ui.popup(&key, || {
        //                     ui.text(help);
        //                 });
        //                 ui.same_line();
        //                 ui.text("(?)");
        //                 if ui.is_item_hovered() {
        //                     ui.open_popup(&key);
        //                 }
        //             }

        //             ui.new_line();
        //             ui.separator();
        //             continue;
        //         }

        //         if let Some(s) = p
        //             .wire_data
        //             .as_ref()
        //             .and_then(|d| bincode::deserialize::<String>(&d).ok())
        //         {
        //             tc.store_kv(&p.field_name, (s.to_string(), s, p.clone()));
        //         } else {
        //             ui.text("Unable to read field as a String");
        //             ui.text(format!("{:#?}", p));
        //             ui.text(format!("{:#?}", widget));
        //         }
        //     }

        //     if !changes.is_empty() {
        //         tc.spawn(|mut tc| async move {
        //             if let Some(mut updates) = tc.cached_mut::<FrameUpdates>() {
        //                 for change in changes {
        //                     updates.0.fields.push(change);
        //                 }
        //             }
        //             Ok(tc)
        //         });
        //     }
        // }
    }
}

/// Select a resource by address to open in the inspection window,
///
#[derive(Reality, Debug, Default, Clone)]
#[reality(call=create_inspect_action, plugin, group="ui")]
pub struct Inspect {
    /// Address of the resource to inspect,
    ///
    #[reality(derive_fromstr)]
    address: Address,
}

async fn create_inspect_action(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<Inspect>().await;

    if let Some(eh) = tc.engine_handle().await {
        // Create a remote action based on a Host resource
        let action = RemoteAction.build::<Host>(tc).await;

        // Returns CallOutput::Update if the address contains a ui node
        let action = action.bind("show_ui_node", |tc| {
            if let Some(_) = tc.cached::<UiNode>() {
                tc.update()
            } else {
                tc.skip()
            }
        });

        // Returns CallOutput::Update if the address contains a ui type node
        let action = action.bind("show_ui_type_node", |tc| {
            if let Some(_) = tc.cached::<UiTypeNode>() {
                tc.update()
            } else {
                tc.skip()
            }
        });

        // Returns CallOutput::Update if the address contains an aux ui type node
        let action = action.bind("show_aux_ui_node", |tc| {
            if let Some(_) = tc.cached::<AuxUiNode>() {
                tc.update()
            } else {
                tc.skip()
            }
        });

        let storage = action.storage.storage.clone();
        let mut storage = storage.write().await;
        init.pack(storage.deref_mut());

        let _inspect = Inspect::default().unpack(storage.deref_mut());

        let published = action.publish(eh).await?;

        eprintln!("published inpsect context -- {published}");

        Ok(())
    } else {
        Err(anyhow!("An engine was not bound to the thunk context."))
    }
}

impl<T: Plugin> UiDisplayMut for FieldRef<T, String, String>
where
    T::Virtual: NewFn<Inner = T>,
{
    fn fmt(&mut self, ui: &super::UiFormatter<'_>) -> anyhow::Result<()> {
        let ui = &ui.imgui;

        if self.is_pending() {
            if self.edit_value(|name, v| {
                let mut editing = v.to_string();
                ui.input_text(name, &mut editing).build();

                if v.as_str() != editing.as_str() {
                    *v = editing;
                    return true;
                }

                false
            }) {
                Ok(())
            } else {
                Err(anyhow!("No changes"))
            }
        } else {
            self.view_value(|v| {
                ui.text(v);
            });

            ui.same_line();
            if ui.button("Edit field") {
                self.pending();
                Ok(())
            } else {
                Err(anyhow!("No changes"))
            }
        }
    }
}

impl<T: Plugin> UiDisplayMut for FieldRef<T, Decorated<String>, Decorated<String>>
where
    T::Virtual: NewFn<Inner = T>,
{
    fn fmt(&mut self, ui: &super::UiFormatter<'_>) -> anyhow::Result<()> {
        let ui = &ui.imgui;

        // TODO -- Switch modes depending on the field ref condition
        /*
        -   If the field ref is pending, that means that it can be edited
        -   If the field is committed or initial, then changes requires a "Confirmation"
        -   If the field is default .. then they can be "added" -> Pending
         */

        if self.is_pending() {
            if self.edit_value(|name, v| {
                if let Some(value) = v.value.as_mut() {
                    let mut editing = value.to_string();
                    ui.input_text(name, &mut editing).build();

                    if value.as_str() != editing.as_str() {
                        *value = editing;
                        return true;
                    }
                }
                false
            }) {
                Ok(())
            } else {
                Err(anyhow!("No changes"))
            }
        } else {
            self.view_value(|v| {
                if let Some(v) = v.value() {
                    ui.text(v);
                }
            });

            ui.same_line();
            if ui.button("Edit field") {
                self.pending();
            }
            Ok(())
        }

        // handle decorations?
    }
}

impl<T: Plugin> UiDisplayMut for FieldRef<T, Decorated<String>, Vec<Decorated<String>>> {
    fn fmt(&mut self, ui: &super::UiFormatter<'_>) -> anyhow::Result<()> {
        todo!()
    }
}
