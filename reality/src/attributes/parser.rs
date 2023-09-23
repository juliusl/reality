use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::debug;
use tracing::trace;

use runmd::prelude::*;

use super::Container;
use super::AttributeTypeParser;
use super::StorageTarget;
use crate::value::v2::ValueContainer;
use crate::AttributeType;

/// Resource for storing custom attribute parse functions
///
#[derive(Clone)]
pub struct CustomAttributeContainer<S: StorageTarget>(HashSet<AttributeTypeParser<S>>);

/// Parser for parsing attributes,
///
#[derive(Default)]
pub struct AttributeParser<Storage: StorageTarget> {
    /// Resource Id of the output container,
    ///
    resource_id: u64,
    /// Id of this attribute,
    ///
    id: <Storage::Attribute as Container>::Id,
    /// Defaults to Value::Empty
    ///
    value: <Storage::Attribute as Container>::Value,
    /// Attribute name, can also be referred to as tag
    ///
    name: Option<String>,
    /// Transient symbol,
    ///
    symbol: Option<String>,
    /// Transient value
    ///
    edit: Option<<Storage::Attribute as Container>::Value>,
    /// Parsed attribute stack,
    ///
    /// **Note**: The first attribute parsed by this parser has a couple of special properties.
    ///
    /// - The value cannot be changed, meaning that the first attribute is **stable**
    /// - Subsequent attributes are considered "properties" of this attribute
    ///     - The value of these properties can be edited without committing the edited-value, this means that there are two values stored for properties
    /// - Although the stable attribute value cannot be changed, it can be replaced, however this should be considered a "fork". **This parser does not manage past states**
    ///
    parsed: Vec<Storage::Attribute>,
    /// Table of attribute parsers,
    ///
    attribute_table: HashMap<String, AttributeTypeParser<Storage>>,
    /// Reference to centralized-storage,
    ///
    storage: Option<Arc<Storage>>,
}

impl<S: StorageTarget> Clone for AttributeParser<S> {
    fn clone(&self) -> Self {
        Self {
            resource_id: self.resource_id.clone(),
            id: self.id.clone(),
            value: self.value.clone(),
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            edit: self.edit.clone(),
            parsed: self.parsed.clone(),
            attribute_table: self.attribute_table.clone(),
            storage: self.storage.clone(),
        }
    }
}

impl<S: StorageTarget> std::fmt::Debug for AttributeParser<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttributeParser")
            .field("id", &self.id)
            .field("value", &self.value)
            .field("name", &self.name)
            .field("symbol", &self.symbol)
            .field("edit", &self.edit)
            .field("parsed", &self.parsed)
            .field("properties", &self.parsed)
            .field("attribute_table", &self.attribute_table)
            .field("storage", &self.storage.is_some())
            .finish()
    }
}

impl<S: StorageTarget> AttributeParser<S> {
    /// Sets the resource id for this parser,
    ///
    pub fn set_resource_id(&mut self, resource_id: u64) {
        self.resource_id = resource_id;
    }

    /// Adds a custom attribute parser and returns self,
    ///
    pub fn with_custom<C>(&mut self) -> &mut Self
    where
        C: AttributeType<S>,
    {
        self.add_custom(AttributeTypeParser::new::<C>());
        self
    }

    /// Adds a custom attribute parser,
    ///
    pub fn add_custom(&mut self, custom_attr: impl Into<AttributeTypeParser<S>>) {
        let custom_attr = custom_attr.into();
        self.attribute_table
            .insert(custom_attr.ident(), custom_attr);
    }

    /// Adds a custom attribute parser,
    ///
    /// Returns a clone of the custom attribute added,
    ///
    pub fn add_custom_with(
        &mut self,
        ident: impl AsRef<str>,
        parse: fn(&mut AttributeParser<S>, String),
    ) -> AttributeTypeParser<S> {
        let attr = AttributeTypeParser::new_with(ident, parse);
        self.add_custom(attr.clone());
        attr
    }

    /// Returns the next attribute from the stack
    ///
    pub fn next(&mut self) -> Option<S::Attribute> {
        self.parsed.pop()
    }

    /// Sets the id for the current parser
    ///
    pub fn set_id(&mut self, id: <S::Attribute as Container>::Id) {
        self.id = id;
    }

    /// Sets the current name value
    ///
    pub fn set_name(&mut self, name: impl AsRef<str>) {
        self.name = Some(name.as_ref().to_string());
    }

    /// Sets the current symbol value
    ///
    pub fn set_symbol(&mut self, symbol: impl AsRef<str>) {
        self.symbol = Some(symbol.as_ref().to_string());
    }

    /// Sets the current value for the parser
    ///
    pub fn set_value(&mut self, value: impl Into<<S::Attribute as Container>::Value>) {
        self.value = value.into();
    }

    /// Sets the current transient value for the parser
    ///
    pub fn set_edit(&mut self, value: impl Into<<S::Attribute as Container>::Value>) {
        self.edit = Some(value.into());
    }

    /// Sets the current storage,
    ///
    pub fn set_storage(&mut self, storage: Arc<S>) {
        self.storage = Some(storage);
    }

    /// Returns the name,
    ///
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    /// Returns the symbol,
    ///
    pub fn symbol(&self) -> Option<&String> {
        self.symbol.as_ref()
    }

    /// Returns the current value,
    ///
    pub fn value(&self) -> &<S::Attribute as Container>::Value {
        &self.value
    }

    /// Returns the last property on the stack,
    ///
    pub fn peek(&self) -> Option<&S::Attribute> {
        self.parsed.last()
    }

    /// Returns an immutable reference to centralized-storage,
    ///
    pub fn storage(&self) -> Option<&Arc<S>> {
        self.storage.as_ref()
    }

    /// Returns an iterator over special attributes installed on this parser,
    ///
    pub fn iter_special_attributes(&self) -> impl Iterator<Item = (&String, &AttributeTypeParser<S>)> {
        self.attribute_table.iter()
    }

    /// Defines a property for the current name,
    ///
    /// Panics if a name is not set.
    ///
    pub fn define(
        &mut self,
        symbol: impl AsRef<str>,
        value: impl Into<<S::Attribute as Container>::Value>,
    ) {
        self.set_symbol(symbol);
        self.set_edit(value.into());
        self.set_value(<S::Attribute as Container>::Value::default());
        self.parse_attribute();
    }

    /// Defines the current attribute,
    ///
    /// Implements the `add` keyword
    ///
    pub fn add(
        &mut self,
        name: impl AsRef<str>,
        value: impl Into<<S::Attribute as Container>::Value>,
    ) {
        self.set_name(name);
        self.set_value(value.into());
        self.parse_attribute();
    }

    /// Pushes an attribute on to the property stack,
    ///
    /// This method can be used directly when configuring a child entity.
    ///
    pub fn define_child(
        &mut self,
        entity: impl Into<<S::Attribute as Container>::Id>,
        symbol: impl AsRef<str>,
        value: impl Into<<S::Attribute as Container>::Value>,
    ) {
        let id = self.id;
        self.set_id(entity.into());
        self.set_symbol(symbol);
        self.set_edit(value.into());
        self.set_value(<S::Attribute as Container>::Value::default());
        self.parse_attribute();
        self.set_id(id);
    }

    /// Parses the current state into an attribute, pushes onto stack
    ///
    fn parse_attribute(&mut self) {
        let value = self.value.clone();
        let name = self.name.clone();
        let symbol = self.symbol.take();
        let edit = self.edit.take();

        match (name, symbol, value, edit) {
            (Some(name), Some(symbol), value, Some(edit)) => {
                if let Some(mut attr) = S::Attribute::new(self.id, symbol, Some(&name), Some(value))
                {
                    attr.edit(edit);
                    self.parsed.push(attr);
                }
            }
            (Some(name), None, value, None) => {
                if let Some(attr) = S::Attribute::new(self.id, name, None, Some(value)) {
                    self.parsed.push(attr);
                }
            }
            _ => {}
        }
    }

    /// Returns the entity that owns this parser,
    ///
    pub fn entity(&self) -> Option<u64> {
        self.storage().map(|s| s.entity(self.id))
    }

    /// Returns the last entity on the stack,
    /// 
    pub fn last_child_entity(&self) -> Option<u64> {
        match self.peek().and_then(|p| Some(p.id())) {
            Some(ref child) if (*child != self.id) => self.storage().map(|s| s.entity(*child)),
            _ => None,
        }
    }
}

#[runmd::prelude::async_trait]
impl<S: StorageTarget<Attribute = crate::Attribute> + Send + Sync + 'static> ExtensionLoader for super::AttributeParser<S> {
    async fn load_extension(&self, extension: &str, input: Option<&str>) -> Option<BoxedNode> {
        let mut parser = self.clone();

        // Forwards-compatibility support
        const V1_ATTRIBUTES_EXTENSION: &'static str = "application/repo.reality.attributes.v1";
        if extension == V1_ATTRIBUTES_EXTENSION {
            debug!("Enabling v1 attribute parsers");
            parser.with_custom::<ValueContainer<bool>>()
                .with_custom::<ValueContainer<i32>>()
                .with_custom::<ValueContainer<[i32; 2]>>()
                .with_custom::<ValueContainer<[i32; 3]>>()
                .with_custom::<ValueContainer<f32>>()
                .with_custom::<ValueContainer<[f32; 2]>>()
                .with_custom::<ValueContainer<[f32; 3]>>()
                .with_custom::<ValueContainer<String>>()
                .with_custom::<ValueContainer<&'static str>>()
                .with_custom::<ValueContainer<Vec<u8>>>()
                .with_custom::<ValueContainer<BTreeSet<String>>>();
            // parser.add_custom_with("true", |parser, _| parser.set_value(true));
            // parser.add_custom_with("false", |parser, _| parser.set_value(false));
            parser.add_custom_with("int_pair", ValueContainer::<[i32; 2]>::parse);
            parser.add_custom_with("int_range", ValueContainer::<[i32; 3]>::parse);
            parser.add_custom_with("float_pair", ValueContainer::<[f32; 2]>::parse);
            parser.add_custom_with("float_range", ValueContainer::<[f32; 3]>::parse);
        }

        // V2 "Plugin" model
        // application/repo.reality.attributes.v1.custom.<plugin-name>;
        /*
            <application/repo.reality.attributes.v1.custom>
            <..request>
        */
        if extension.starts_with("application/repo.reality.attributes.v1.custom") {
            if let Some((prefix, plugin)) = extension.rsplit_once('.') {
                if prefix == "application/repo.reality.attributes.v1" && plugin == "custom" {
                    if let Some(storage) = self.storage() {
                        if let Some(attributes) = storage.resource::<CustomAttributeContainer<S>>(None) {
                            debug!("Loading plugins");
                            for plugin in attributes.0.iter() {
                                parser.add_custom(plugin.clone());
                            }

                            // Increment the id since this is a new extension
                            parser.id += 1;
                        }
                    }
                } else if prefix == "application/repo.reality.attributes.v1.custom" {
                    if let Some(plugin) = self.attribute_table.get(plugin) {
                        plugin.parse(&mut parser, input.unwrap_or_default());
                    }
                } else {
                }
            }
        }

        Some(Box::pin(parser))
    }
}

impl<S: StorageTarget<Attribute = crate::Attribute> + Send + Sync + 'static> Node for super::AttributeParser<S> {
    fn set_info(&mut self, node_info: NodeInfo, _block_info: BlockInfo) {
        if self.id == 0 {
            if let Some(parent) = node_info.parent_idx.as_ref() {
                self.set_id(*parent as u32);
            } else {
                self.set_id(node_info.idx as u32);
            }
        } else {
            // Only set if this node originates from loading a custom plugin extension
        }
    }

    fn define_property(&mut self, name: &str, tag: Option<&str>, input: Option<&str>) {
        if let Some(tag) = tag.as_ref() {
            self.set_name(tag);
            self.set_symbol(name);
        } else {
            self.set_name(name);
        }

        match self.attribute_table.get(name).cloned() {
            Some(cattr) => {
                cattr.parse(self, input.unwrap_or_default());
                self.parse_attribute();
            }
            None => {
                trace!(attr_ty = name, "Did not have attribute");
            }
        }
    }

    fn completed(mut self: Box<Self>) {
        let mut attrs = vec![];
        while let Some(next) = self.next() {
            attrs.push(next);
        }

        // if let Some(storage) = self.storage() {
        //     if let Some(mut block) = storage.resource_mut::<Block>(Some(self.resource_id)) {
        //         for attr in attrs {
        //             block.add_attribute(&attr);
        //         }
        //     } else {
        //         error!("Block does not exist, {}", self.resource_id);
        //     }
        // }
    }
}

// #[tokio::test]
// async fn test_v2_parser() {
//     let parser = AttributeParser::<World>::default();

//     let mut parser = parser
//         .load_extension("application/repo.reality.attributes.v1", None)
//         .await
//         .expect("should return a node");
//     parser.define_property("int", Some("test"), Some("256"));
//     parser.define_property("float", Some("test-2"), Some("256.0"));

//     println!("{:#?}", parser);
// }
