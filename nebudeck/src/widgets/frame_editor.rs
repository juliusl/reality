use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ops::Deref;

use imgui::TableColumnFlags;
use imgui::TableColumnSetup;
use imgui::TableFlags;
use imgui::TreeNodeFlags;
use loopio::prelude::*;
use tracing::info;
use tracing::trace;

use crate::ext::imgui_ext::ImguiExt;

/// Widget to edit frames of an attribute,
///
#[derive(Reality, Debug, Default, Clone)]
#[reality(call = enable_frame_editor, plugin, rename = "frame-editor")]
pub struct FrameEditor {
    /// Path to the attribute being edited,
    ///
    #[reality(derive_fromstr)]
    path: Address,
    /// Name of the editor,
    ///
    #[reality(option_of=String)]
    editor_name: Option<String>,
    /// Map of panels,
    ///
    #[reality(map_of=Decorated<String>)]
    panel: BTreeMap<String, Decorated<String>>,
    /// List of properties to edit,
    ///
    #[reality(vec_of=Decorated<String>)]
    edit: Vec<Decorated<String>>,
    /// Action button,
    ///
    #[reality(set_of=Decorated<String>)]
    action: BTreeSet<Decorated<String>>,
}

async fn enable_frame_editor(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let mut init = Remote.create::<FrameEditor>(tc).await;

    init.editor_name = init.editor_name.or(tc.property("title"));

    let eh = tc.engine_handle().await.expect("not bound to an engine");
    info!("Enabling frame editor -- {:?}", init.path);
    if let Ok(mut editing) = eh.hosted_resource(&init.path.to_string()).await {
        let editing = editing.context_mut();

        editing.write_cache(init.clone());

        info!("Found path -- {:?}", editing.attribute);
        // If enabled, allows available field packets to be decoded,
        {
            let node = editing.node().await;
            if let Some(bus) = node.current_resource::<WireBus>(editing.attribute.transmute()) {
                info!("Found wire bus");
                drop(node);
                editing.maybe_write_cache(bus);
            }
        }

        // If set, allows the ability to apply frame updates. (**Note** The receiving end must enable updating)
        {
            let node = editing.node().await;
            if let Some(change_pipeline) =
                node.current_resource::<FrameUpdates>(editing.attribute.transmute())
            {
                info!("Found change pipeline");
                drop(node);
                editing.maybe_write_cache(change_pipeline);
            }
        }

        let recv = editing.initialized_frame().await.recv.clone();
        // Gets the current parsed attributes state of the target attribute,
        {
            let node = editing.node().await;
            if let Some(parsed_attributes) =
                node.current_resource::<ParsedNode>(loopio::prelude::ResourceKey::root())
            {
                info!("Found parsed attributes");
                drop(node);
                editing.maybe_write_cache(parsed_attributes);

                if let Some(parsed) = editing.cached::<ParsedNode>() {
                    // parsed.index_decorations(editing.attribute, editing).await;
                    editing.store_kv(editing.attribute, recv);
                }
            }
        }

        editing.push_ui_node(move |_ui| {
            if let Ok(mut tc) = _ui.tc.lock() {
                let attr = tc.get().unwrap().attribute;

                let mut init = tc.get_mut().unwrap().cached::<FrameEditor>().unwrap();
                let title = init
                    .editor_name
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", attr));

                let ui = &_ui.imgui;
                ui.window(format!("Frame Editor - {}", title))
                    .size([800.0, 600.0], imgui::Condition::Once)
                    .build(|| {
                        ui.label_text("Resource Key", attr.key().to_string());

                        // if ui.collapsing_header("DEBUG --", TreeNodeFlags::empty()) {
                        //     ui.text(debug);
                        // }

                        // Map of field metadata settings compiled to wire bus,
                        //
                        let mut field_map = BTreeMap::<String, Vec<FieldPacket>>::new();

                        if let Some(wb) = tc.get().unwrap().cached_ref::<WireBus>() {
                            for packet in wb.packets().iter() {
                                let entry =
                                    field_map.entry(packet.field_name.to_string()).or_default();
                                entry.push(packet.clone());
                            }

                            // Shows the frames available in the wire bus,
                            //
                            if ui.collapsing_header("Wire Bus", TreeNodeFlags::empty()) {
                                let table_flags: TableFlags = TableFlags::REORDERABLE
                                    | TableFlags::HIDEABLE
                                    | TableFlags::RESIZABLE
                                    | TableFlags::NO_BORDERS_IN_BODY
                                    | TableFlags::SIZING_STRETCH_PROP;

                                if let Some(_h) = ui.begin_table_header_with_flags(
                                    "table",
                                    [
                                        "field_offset",
                                        "field_name",
                                        "type_name",
                                        "type_size",
                                        "owner_name",
                                        "attribute_hash",
                                    ]
                                    .map(TableColumnSetup::new)
                                    .map(|mut s| {
                                        if s.name == "owner_name"
                                            || s.name == "attribute_hash"
                                            || s.name == "field_offset"
                                        {
                                            s.flags = TableColumnFlags::DEFAULT_HIDE
                                        } else if s.name == "type_name" {
                                            s.flags = TableColumnFlags::WIDTH_STRETCH;
                                        }
                                        s
                                    }),
                                    table_flags,
                                ) {
                                    for (_, packet) in wb.packets().iter().enumerate() {
                                        let FieldPacket {
                                            data_type_name,
                                            data_type_size,
                                            field_offset,
                                            owner_name,
                                            field_name,
                                            attribute_hash,
                                            ..
                                        } = packet;

                                        ui.table_next_column();
                                        ui.text(field_offset.to_string());

                                        ui.table_next_column();
                                        ui.text(field_name);

                                        ui.table_next_column();
                                        ui.text(data_type_name);

                                        ui.table_next_column();
                                        ui.text(data_type_size.to_string());

                                        ui.table_next_column();
                                        ui.text(owner_name);

                                        ui.table_next_column();
                                        ui.text(attribute_hash.unwrap_or_default().to_string());
                                    }
                                }
                            }
                        }

                        defined_properties_section(tc.get().unwrap(), ui);

                        let mut queue_update = false;
                        if let Some(queued) = tc.get().unwrap().cached_ref::<FrameUpdates>() {
                            let mut render = vec![];
                            for (idx, q) in queued.frame.fields.iter().enumerate() {
                                let FieldPacket {
                                    wire_data,
                                    data_type_name,
                                    data_type_size,
                                    field_offset,
                                    field_name,
                                    owner_name,
                                    attribute_hash,
                                    ..
                                } = q;

                                render.push(move || {
                                    ui.text(format!("Field Packet {idx}:"));
                                    ui.label_text("field_name", field_name);
                                    ui.label_text("type_name", data_type_name);
                                    ui.label_text("type_size", data_type_size.to_string());
                                    ui.label_text("field_offset", field_offset.to_string());
                                    ui.label_text("owner_name", owner_name);
                                    ui.label_text(
                                        "attribute hash",
                                        format!("{:?}", attribute_hash),
                                    );

                                    if let Some(bin) = wire_data {
                                        ui.text("Has binary data");
                                        if ui.button("Deserialize") {
                                            if let Ok(s) = bincode::deserialize::<String>(&bin) {
                                                println!("Deserialized: {s}");
                                            }
                                        }
                                    }
                                })
                            }

                            if ui.collapsing_header("Queued Changes", TreeNodeFlags::empty()) {
                                if !render.is_empty() && ui.button("Submit") {
                                    queue_update = true;
                                }

                                for r in render.drain(..) {
                                    r();
                                }
                            }
                        }

                        for (name, panel) in init.panel.iter() {
                            if ui.collapsing_header(
                                panel.value.as_ref().unwrap_or(name),
                                TreeNodeFlags::DEFAULT_OPEN,
                            ) {
                                // if let Some(deco) = panel() {
                                //     if let Some(docs) = deco.docs() {
                                //         for d in docs {
                                //             ui.text(d);
                                //         }
                                //         ui.new_line();
                                //     }
                                // }

                                ui.indent();
                                // This can be optimized later -
                                for edit in init.edit.iter_mut() {
                                    if !tc.get().unwrap().kv_contains::<FieldWidget>(&edit) {
                                        match edit {
                                            Decorated { ref value, .. } => {
                                                if let Some(value) = value.as_ref() {
                                                    let properties = ["title", "widget", "help"]
                                                        .map(|d| edit.property(d));
                                                    match properties {
                                                        [title, Some(widget), help] => {
                                                            let editor = FieldWidget {
                                                                title: title
                                                                    .unwrap_or(value.to_string()),
                                                                widget: widget.to_string(),
                                                                field: value.to_string(),
                                                                field_map: field_map.clone(),
                                                                doc_headers: edit
                                                                    .doc_headers()
                                                                    .unwrap_or_default(),
                                                                help: help
                                                                    .as_ref()
                                                                    .map(String::to_string),
                                                                widget_table: tc
                                                                    .get()
                                                                    .unwrap()
                                                                    .cached::<EditorWidgetTable>()
                                                                    .unwrap_or_default(),
                                                            };
                                                            tc.get_mut()
                                                                .unwrap()
                                                                .store_kv(&edit, editor);
                                                        }
                                                        _ => {
                                                            ui.label_text(
                                                                value,
                                                                "Widget is not specified",
                                                            );
                                                        }
                                                    }
                                                } else {
                                                    ui.text("Value is not initialized");
                                                }
                                            }
                                        }
                                    } else if let Some((_, mut editor)) =
                                        tc.get_mut().unwrap().take_kv::<FieldWidget>(&edit)
                                    {
                                        editor.show(tc.get_mut().unwrap(), ui);
                                        tc.get_mut().unwrap().store_kv(&edit, editor);
                                    } else {
                                        ui.text("Could not initialize field widget");
                                    }
                                }

                                // List of actions
                                ui.new_line();
                                for (panel_name, field) in
                                    init.action.iter().map(|t| (t.tag(), t.value()))
                                {
                                    action_button(
                                        panel_name,
                                        field,
                                        name,
                                        ui,
                                        tc.get_mut().unwrap(),
                                    );
                                }

                                ui.unindent();
                            }
                        }

                        if queue_update {
                            trace!("Queued frame update");
                            let rk = tc.get().unwrap().cached::<ResourceKey<Attribute>>();

                            if let Some(cache) = tc.get_mut().unwrap().take_cache::<FrameUpdates>()
                            {
                                tc.get().unwrap().spawn(move |tc| async move {
                                    unsafe {
                                        println!("Outside: {:?}", &rk);
                                        println!(
                                            "Putting frame change -- {:?} packets: {}",
                                            tc.attribute,
                                            cache.frame.fields.len()
                                        );
                                        println!("{:#?}", cache);
                                        tc.node().await.lazy_put_resource::<FrameUpdates>(
                                            *cache,
                                            tc.attribute.transmute(),
                                        );
                                        tc.process_node_updates().await;
                                    }
                                    Ok(tc)
                                });

                                tc.get_mut().unwrap().write_cache(FrameUpdates::default());
                            }
                        }
                    });
            }

            true
        });
    }

    Ok(())
}

/// Contains a set of relevant properties for a field widget,
///
#[derive(Debug)]
pub struct FieldWidget {
    /// Title for the widget be set w/ a comment property,
    ///
    /// |# title = Title
    ///
    pub title: String,
    /// Widget name,
    ///
    /// |# widget = text
    ///
    pub widget: String,
    /// Help message that can be displayed to assist with operating
    /// this widget.
    ///
    pub help: Option<String>,
    /// Documentation headers stored w/ this field widget,
    ///
    pub doc_headers: Vec<String>,
    /// Field name,
    ///
    pub field: String,
    /// Map of field metadata,
    ///
    pub field_map: BTreeMap<String, Vec<FieldPacket>>,
    /// Table of widgets that can be used,
    ///
    /// TODO: This could be generic.
    ///
    pub widget_table: EditorWidgetTable,
}

/// Table of widget functions,
///
#[derive(Clone, Debug)]
pub struct EditorWidgetTable {
    /// Map of widget functions,
    ///
    widget_table: BTreeMap<String, fn(&mut FieldWidget, &mut ThunkContext, &imgui::Ui)>,
}

impl EditorWidgetTable {
    /// Returns true if a widget already exists,
    ///
    pub fn is_registered(&self, name: impl AsRef<str>) -> bool {
        self.widget_table.contains_key(name.as_ref())
    }

    /// Registers a new widget func,
    ///
    /// Widgets cannot be replaced after being added.
    ///
    pub fn register(
        &mut self,
        name: impl Into<String>,
        func: fn(&mut FieldWidget, &mut ThunkContext, &imgui::Ui),
    ) {
        self.widget_table.entry(name.into()).or_insert(func);
    }

    /// Show a widget,
    ///
    fn show(&self, widget: &mut FieldWidget, tc: &mut ThunkContext, ui: &imgui::Ui) {
        if let Some(show_fn) = self.widget_table.get(&widget.widget) {
            show_fn(widget, tc, ui)
        } else {
            ui.text(format!("Could not find widget: {:?}", widget.widget));
        }
    }

    /// Creates a new widget table,
    ///
    pub fn new() -> Self {
        let mut table = Self {
            widget_table: BTreeMap::new(),
        };

        // Simple UI fields
        table.register("input_text", |widget, tc, ui| {
            if let Some(field) = widget.field_map.get(&widget.field) {
                let mut changes = vec![];

                // Iterate through each discovered packet,
                for p in field.iter() {
                    if let Some((f, mut field)) =
                        tc.fetch_mut_kv::<(String, String, FieldPacket)>(&p.field_name)
                    {
                        // Display any doc headers before this
                        for d in &widget.doc_headers {
                            ui.text(d);
                        }

                        // Enables text editing input
                        ui.input_text(&widget.title, &mut field.1).build();

                        // Modification actions
                        if field.0 != field.1 {
                            ui.text("Value has changed");
                            ui.same_line();
                            if ui.button("Reset") {
                                field.1 = field.0.to_string();
                            }

                            ui.same_line();
                            if ui.button("Save Changes") {
                                field.0 = field.1.to_string();
                                if let (mut field, Ok(wire)) =
                                    (field.2.clone(), bincode::serialize(&field.0.deref()))
                                {
                                    field.wire_data = Some(wire);
                                    changes.push(field);
                                }
                            }
                        }

                        // Show the help popup
                        let key = format!("help_popup##{}", f.key());
                        if let Some(help) = widget.help.as_ref() {
                            ui.popup(&key, || {
                                ui.text(help);
                            });
                            ui.same_line();
                            ui.text("(?)");
                            if ui.is_item_hovered() {
                                ui.open_popup(&key);
                            }
                        }

                        ui.new_line();
                        ui.separator();
                        continue;
                    }

                    if let Some(s) = p
                        .wire_data
                        .as_ref()
                        .and_then(|d| bincode::deserialize::<String>(&d).ok())
                    {
                        tc.store_kv(&p.field_name, (s.to_string(), s, p.clone()));
                    } else {
                        ui.text("Unable to read field as a String");
                        ui.text(format!("{:#?}", p));
                        ui.text(format!("{:#?}", widget));
                    }
                }

                if !changes.is_empty() {
                    tc.spawn(|mut tc| async move {
                        if let Some(mut updates) = tc.cached_mut::<FrameUpdates>() {
                            for change in changes {
                                updates.frame.fields.push(change);
                            }
                        }
                        Ok(tc)
                    });
                }
            }
        });

        table.register("input_float", |widget, tc, ui| {
            if let Some(field) = widget.field_map.get(&widget.field) {
                let mut changes = vec![];

                // Iterate through each discovered packet,
                for p in field.iter() {
                    if let Some((f, mut field)) =
                        tc.fetch_mut_kv::<(f32, f32, FieldPacket)>(&p.field_name)
                    {
                        // Display any doc headers before this
                        for d in &widget.doc_headers {
                            ui.text(d);
                        }

                        // Enables text editing input
                        ui.input_float(&widget.title, &mut field.1).build();

                        // Modification actions
                        if field.0 != field.1 {
                            ui.text("Value has changed");
                            ui.same_line();
                            if ui.button("Reset") {
                                field.1 = field.0;
                            }

                            ui.same_line();
                            if ui.button("Save Changes") {
                                field.0 = field.1;
                                if let (mut field, Ok(wire)) =
                                    (field.2.clone(), bincode::serialize(&field.0))
                                {
                                    field.wire_data = Some(wire);
                                    changes.push(field);
                                }
                            }
                        }

                        // Show the help popup
                        let key = format!("help_popup##{}", f.key());
                        if let Some(help) = widget.help.as_ref() {
                            ui.popup(&key, || {
                                ui.text(help);
                            });
                            ui.same_line();
                            ui.text("(?)");
                            if ui.is_item_hovered() {
                                ui.open_popup(&key);
                            }
                        }

                        ui.new_line();
                        ui.separator();
                        continue;
                    }

                    if let Some(s) = p
                        .wire_data
                        .as_ref()
                        .and_then(|d| bincode::deserialize::<f32>(&d).ok())
                    {
                        tc.store_kv(&p.field_name, (s.to_string(), s, p.clone()));
                    } else {
                        ui.text("Unable to read field as a String");
                        ui.text(format!("{:#?}", p));
                        ui.text(format!("{:#?}", widget));
                    }
                }

                if !changes.is_empty() {
                    tc.spawn(|mut tc| async move {
                        if let Some(mut updates) = tc.cached_mut::<FrameUpdates>() {
                            for change in changes {
                                updates.frame.fields.push(change);
                            }
                        }
                        Ok(tc)
                    });
                }
            }
        });

        table
    }
}

impl Default for EditorWidgetTable {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldWidget {
    /// Shows the widget,
    ///
    pub fn show(&mut self, tc: &mut ThunkContext, ui: &imgui::Ui) {
        self.widget_table.clone().show(self, tc, ui)
    }
}

fn defined_properties_section(tc: &ThunkContext, ui: &imgui::Ui) {
    let mut render_properties = vec![];
    let rk = tc.attribute;
    render_properties.push(|| {
        view_decorations(rk, &tc, ui);

        if let Some(fields) = rk.recv().and_then(|r| r.fields()) {
            for prop in fields.iter() {
                view_decorations(ResourceKey::<Property>::with_repr(*prop), tc, ui)
            }
        }
    });

    if ui.collapsing_header("Defined Properties", TreeNodeFlags::empty()) {
        for r in render_properties {
            r();
        }
    }
}

fn view_field<T: std::hash::Hash + Send + Sync + 'static>(
    rk: ResourceKey<T>,
    _: &ThunkContext,
    ui: &imgui::Ui,
) {
    if let Some(packet) = rk.transmute::<Property>().field_packet() {
        if let Some(_t) = ui.tree_node(format!("Field - packet {}", rk.data)) {
            let FieldPacket {
                data_type_name,
                data_type_size,
                field_offset,
                field_name,
                owner_name,
                attribute_hash,
                ..
            } = packet.clone();

            if let Some(_t) = ui.begin_table_header(
                attribute_hash.unwrap_or_default().to_string(),
                ["value", ""].map(TableColumnSetup::new),
            ) {
                ui.table_next_column();
                ui.text(field_offset.to_string());

                ui.table_next_column();
                ui.text("field_offset");

                ui.table_next_column();
                ui.text(field_name);

                ui.table_next_column();
                ui.text("field_name");

                ui.table_next_column();
                ui.text(data_type_name);

                ui.table_next_column();
                ui.text("type_name");

                ui.table_next_column();
                ui.text(data_type_size.to_string());

                ui.table_next_column();
                ui.text("type_size");

                ui.table_next_column();
                ui.text(owner_name);

                ui.table_next_column();
                ui.text("owner_name");

                ui.table_next_column();
                ui.text(rk.data.to_string());

                ui.table_next_column();
                ui.text("attribute hash");
            }
        }
    }
}

/// Visualize decorations for a resource,
///
fn view_decorations<T: std::hash::Hash + Send + Sync + 'static>(
    rk: ResourceKey<T>,
    tc: &ThunkContext,
    ui: &imgui::Ui,
) {
    ui.indent();
    if let Some(_n) = ui.tree_node(format!("Decorations {:?}", rk)) {
        if let Some(docs) = rk.transmute::<Attribute>().doc_headers() {
            for d in docs.iter() {
                ui.text(d);
            }
            ui.new_line();
        }

        if let Some(annotations) = rk.transmute::<Attribute>().annotations() {
            for (name, prop) in annotations.iter() {
                ui.label_text(name, prop);
            }
        }

        view_field(rk, tc, ui);
    }
    ui.unindent();
}

fn action_button(
    panel_name: Option<&String>,
    field: Option<&String>,
    name: &String,
    ui: &imgui::Ui,
    __tc: &mut ThunkContext,
) {
    if let (Some(panel_name), Some(field)) = (panel_name, field) {
        if panel_name == name {
            if !field.is_empty() {
                if ui.button(field) {
                    eprintln!("{} pressed", field);
                }
            } else {
                if ui.button("Start") {
                    eprintln!("start pressed");
                }
            }
        }
    }
}
