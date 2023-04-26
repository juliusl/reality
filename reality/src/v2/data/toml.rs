use std::collections::BTreeSet;
use std::ops::Index;
use std::path::Path;
use std::sync::Arc;

use crate::v2::compiler::BuildLog;
use crate::v2::Block;
use crate::v2::Build;
use crate::v2::Properties;
use crate::v2::Property;
use crate::v2::Root;
use crate::v2::Visitor;
use crate::Error;
use crate::Identifier;
use crate::Value;
use serde::Deserialize;
use specs::Builder;
use specs::Component;
use specs::HashMapStorage;
use specs::World;
use specs::WorldExt;
use toml_edit::table;
use toml_edit::value;
use toml_edit::visit::Visit;
use toml_edit::Array;
use toml_edit::Document;
use toml_edit::InlineTable;
use toml_edit::Item;
use toml_edit::Table;
use tracing::error;
use tracing::trace;

use super::blob::BlobInfo;
use super::query::Predicate;
use super::query::Query;

/// Struct for building a TOML-document from V2 compiler build,
///
#[derive(Default, Component, Clone, Debug)]
#[storage(HashMapStorage)]
pub struct DocumentBuilder {
    /// Current toml doc being built,
    ///
    doc: Document,
    /// Identifier being parsed,
    ///
    parsing: Option<String>,
}

impl DocumentBuilder {
    /// Returns a new document builder,
    ///
    pub fn new() -> Self {
        let mut doc = Self::default();
        let mut table = table();
        table.as_table_mut().map(|t| t.set_implicit(true));
        doc.doc["properties"] = table.clone();
        doc.doc["block"] = table.clone();
        doc.doc["root"] = table.clone();
        doc
    }

    /// Visits the internal document w/ a visit_mut implementation,
    ///
    pub fn visit_mut(&mut self, mut visitor: impl toml_edit::visit_mut::VisitMut) {
        visitor.visit_document_mut(&mut self.doc);
    }

    /// Visits the internal document w/ a visit implementation,
    ///
    pub fn visit<'a>(&'a self, mut visitor: impl toml_edit::visit::Visit<'a>) {
        visitor.visit_document(&self.doc);
    }

    /// Formats an identifier for use in a table header,
    ///
    fn format_ident(ident: &Identifier) -> String {
        match ident.commit() {
            Ok(committed) => format!("{:#}", committed)
                .replace("\"", "")
                .trim_matches('.')
                .to_string(),
            Err(err) => {
                error!("Could not format ident, {err}");
                ident.to_string()
            }
        }
    }
}

impl Visitor for DocumentBuilder {
    fn visit_block(&mut self, block: &crate::v2::Block) {
        let owner = Self::format_ident(block.ident());
        self.doc["block"][&owner] = table();

        let mut roots = Array::new();
        for root in block.roots() {
            roots.push(
                format!("{:#}", root.ident)
                    .replace("\"", "")
                    .trim_matches('.'),
            );
        }

        self.doc["block"][&owner].as_table_mut().map(|t| {
            t.set_implicit(true);
            t["roots"] = value(roots);
        });
    }

    fn visit_root(&mut self, root: &crate::v2::Root) {
        let owner = Self::format_ident(&root.ident);
        if !self.doc["root"]
            .get(&owner)
            .map(|t| t.is_table())
            .unwrap_or_default()
        {
            self.doc["root"][&owner] = table();
        }

        let mut extensions = Array::new();
        for ext in root.extensions() {
            extensions.push(format!("{:#}", ext).replace("\"", "").trim_matches('.'));
        }

        self.doc["root"][&owner].as_table_mut().map(|t| {
            t.set_implicit(true);
            t["extensions"] = value(extensions);
        });
    }

    fn visit_properties(&mut self, properties: &crate::v2::Properties) {
        let owner = Self::format_ident(properties.owner());
        self.doc["properties"][&owner] = table();
        // self.doc["properties"][&owner]
        //     .as_table_mut()
        //     .map(|t| t.set_implicit(true));
        self.parsing = Some(owner);

        for (name, property) in properties.iter_properties() {
            self.visit_property(name, property);
        }
    }

    fn visit_value(&mut self, name: &String, idx: Option<usize>, value: &Value) {
        self.parsing.as_ref().map(|id| {
            if let Some(0) = idx {
                self.doc["properties"][id][name] = toml_edit::value(Array::new());
                let item: Item = value.clone().into();

                if let Some(value) = item.as_value() {
                    self.doc["properties"][id][name]
                        .as_array_mut()
                        .map(|a| a.push(value));
                } else if let Some(table) = item.as_inline_table() {
                    self.doc["properties"][id][name]
                        .as_array_mut()
                        .map(|a| a.push(table.clone()));
                }
            } else if let Some(_) = idx {
                let item: Item = value.clone().into();

                if let Some(value) = item.as_value() {
                    self.doc["properties"][id][name]
                        .as_array_mut()
                        .map(|a| a.push(value));
                }
            } else {
                self.doc["properties"][id][name] = value.clone().into();
            }
        });
    }
}

impl Into<toml_edit::Item> for Value {
    fn into(self) -> toml_edit::Item {
        match self {
            Value::Empty => value(""),
            Value::Bool(b) => value(b),
            Value::TextBuffer(t) => value(t),
            Value::Int(i) => value(i as i64),
            Value::IntPair(i1, i2) => {
                let mut arr = Array::new();
                arr.push(i1 as i64);
                arr.push(i2 as i64);
                value(arr)
            }
            Value::IntRange(i1, i2, i3) => {
                let mut arr = Array::new();
                arr.push(i1 as i64);
                arr.push(i2 as i64);
                arr.push(i3 as i64);
                value(arr)
            }
            Value::Float(f) => value(f as f64),
            Value::FloatPair(f1, f2) => {
                let mut arr = Array::new();
                arr.push(f1 as f64);
                arr.push(f2 as f64);
                value(arr)
            }
            Value::FloatRange(f1, f2, f3) => {
                let mut arr = Array::new();
                arr.push(f1 as f64);
                arr.push(f2 as f64);
                arr.push(f3 as f64);
                value(arr)
            }
            Value::BinaryVector(b) => {
                let mut table = Table::new();
                table["src"] = value("base64");
                table["data"] = value(base64::encode(b));
                value(table.into_inline_table())
            }
            Value::Reference(r) => value(r as i64),
            Value::Symbol(s) => value(s),
            Value::Complex(c) => {
                let mut table = InlineTable::new();
                table["_type"] = "complex".into();
                for (idx, c) in c.iter().enumerate() {
                    table[c] = (idx as i64).into();
                }
                value(table)
            }
        }
    }
}

impl Into<Document> for DocumentBuilder {
    fn into(self) -> Document {
        self.doc
    }
}

impl Into<TomlProperties> for &DocumentBuilder {
    fn into(self) -> TomlProperties {
        TomlProperties {
            doc: Arc::new(self.doc.clone()),
        }
    }
}

/// Component for properties as a toml document,
///
#[derive(Debug, Component, Clone)]
#[storage(HashMapStorage)]
pub struct TomlProperties {
    /// Read-only TOML doc,
    ///
    pub doc: Arc<Document>,
}

impl TomlProperties {
    /// Loads toml properties from a file and returns the result,
    ///
    pub async fn try_load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let file = path.as_ref().canonicalize()?;
        let file = tokio::fs::read_to_string(file).await?;

        let mut doc: Document = file.parse()?;
        let mut table = table();
        table.as_table_mut().map(|t| t.set_implicit(true));
        if !doc.contains_table("properties") {
            doc["properties"] = table.clone();
        }

        if !doc.contains_table("block") {
            doc["block"] = table.clone();
        }

        if !doc.contains_table("root") {
            doc["root"] = table.clone();
        }

        Ok(Self { doc: Arc::new(doc) })
    }

    /// Tries to save toml document to a path,
    ///
    pub async fn try_save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        if let Some(dir) = path.as_ref().parent() {
            tokio::fs::create_dir_all(&dir).await?;

            tokio::fs::write(path, format!("{}", self.doc)).await?;

            Ok(())
        } else {
            Err("Could not save properties".into())
        }
    }

    /// Deserializes properties into T,
    ///
    pub fn deserialize<T: for<'de> Deserialize<'de>>(
        &self,
        ident: &Identifier,
    ) -> Result<T, Error> {
        if let Some(result) = self["properties"]
            .get(DocumentBuilder::format_ident(ident))
            .and_then(|t| t.as_table())
            .map(|t| toml::from_str::<T>(&format!("{}", t)))
        {
            result.map_err(|e| format!("Could no deserialize, {e}").into())
        } else {
            Err(format!("Properties did not exist for {:#}", ident).into())
        }
    }

    /// Deserialize a map created by a key-array from properties,
    ///
    /// Each key in the key array is the key of a value in the properties table, each value is moved
    /// to a new table. The new table will the be deserialized to T.
    ///
    /// # Example
    /// ```norun
    ///  + .host
    /// : RUST_LOG  .env reality=trace
    /// : HOST      .env test.io
    ///
    /// # For types that aren't strings
    /// : env       .symbol TIMEOUT
    /// : TIMEOUT   .int    100
    /// ```
    ///
    /// will derive,
    /// ```norun
    /// [properties."host"]
    /// env      = [ "RUST_LOG", "HOST" ]
    /// RUST_LOG = "reality=trace",
    /// HOST     = "test.io"
    /// TIMEOUT  = 100
    /// ```
    /// when creating a TomlProperties component.
    ///
    /// Another way to interpret this output would be as a map struct,
    ///
    /// Examples (TODO),
    ///
    /// ```norun
    /// js/json
    /// {
    ///     "RUST_LOG": "reality=trace",
    ///     "HOST": "test.io",
    ///     "TIMEOUT": 100
    /// }
    /// ```
    ///
    /// ```norun
    /// #[derive(Default, Serialize, Deserialize)]
    /// struct HostEnv {
    ///     #[serde(rename = "RUST_LOG", default_t = String::from("reality=trace"))]
    ///     rust_log: String,
    ///     #[serde(rename = "HOST", default_t = String::from("test.io"))]
    ///     host: String,
    ///     #[serde(rename = "TIMEOUT", default_t = 100)]
    ///     timeout: i64,
    /// }
    ///```
    ///
    pub fn deserialize_keys<T: for<'de> Deserialize<'de>>(
        &self,
        ident: &Identifier,
        key_arr: impl AsRef<str>,
    ) -> Result<T, Error> {
        if let Some(result) = self["properties"]
            .get(DocumentBuilder::format_ident(ident))
            .and_then(|t| t.as_table())
            .map(|t| {
                let mut table = toml_edit::Table::new();

                t[key_arr.as_ref()].as_array().map(|keys| {
                    for k in keys.iter().filter_map(|k| k.as_str()) {
                        table[k] = t[k].clone();
                    }
                });

                toml::from_str::<T>(&format!("{}", table))
            })
        {
            result.map_err(|e| format!("Could no deserialize, {e}").into())
        } else {
            Err(format!("Properties did not exist for {:#}", ident).into())
        }
    }
}

impl<'a> Index<&'a str> for TomlProperties {
    type Output = toml_edit::Item;

    fn index(&self, index: &'a str) -> &Self::Output {
        &self.doc[index]
    }
}

impl<'a> Query<'a> for TomlProperties {
    fn query(
        &'a self,
        pat: impl AsRef<str>,
        predicate: impl Predicate + 'static,
    ) -> Result<super::query::QueryIter<'a>, Error> {
        let mut props = vec![];
        self.doc["properties"].as_table().map(|t| {
            for (key, item) in t.iter() {
                if let Some(ident) = key.parse::<Identifier>().ok() {
                    let mut properties = Properties::new(ident);

                    item.as_table().map(|t| {
                        for (key, item) in t.iter() {
                            if let Some(value) = item.as_value() {
                                properties.add(key, value);
                            }
                        }
                    });

                    props.push(properties);
                }
            }
        });

        let pat = pat.as_ref().to_string();

        Ok(Box::new(props.into_iter().filter_map(move |p| {
            if let Ok(mut result) = p.query(&pat, predicate) {
                result.next()
            } else {
                None
            }
        })))
    }
}

impl<'a> Into<crate::v2::Property> for &'a toml_edit::Value {
    fn into(self) -> crate::v2::Property {
        let value: Value = (&self.clone()).into();

        match self {
            toml_edit::Value::Array(arr) if value == Value::Empty => crate::v2::property_list(arr),
            _ => crate::v2::property_value(value),
        }
    }
}

impl<'a> Into<crate::Value> for &'a toml_edit::Value {
    fn into(self) -> crate::Value {
        match self {
            toml_edit::Value::String(s) => {
                crate::Value::Symbol(s.to_string().trim().trim_matches('"').to_string())
            }
            toml_edit::Value::Integer(i) => crate::Value::Int(*i.value() as i32),
            toml_edit::Value::Float(f) => crate::Value::Float(*f.value() as f32),
            toml_edit::Value::Boolean(b) => crate::Value::Bool(*b.value()),
            toml_edit::Value::Datetime(d) => crate::Value::Symbol(d.to_string()),
            toml_edit::Value::Array(arr) => match (arr.get(0), arr.get(1), arr.get(2)) {
                (Some(toml_edit::Value::Integer(a)), Some(toml_edit::Value::Integer(b)), None) => {
                    let a = *a.value() as i32;
                    let b = *b.value() as i32;
                    Value::IntPair(a, b)
                }
                (
                    Some(toml_edit::Value::Integer(a)),
                    Some(toml_edit::Value::Integer(b)),
                    Some(toml_edit::Value::Integer(c)),
                ) => {
                    let a = *a.value() as i32;
                    let b = *b.value() as i32;
                    let c = *c.value() as i32;
                    Value::IntRange(a, b, c)
                }
                (Some(toml_edit::Value::Float(a)), Some(toml_edit::Value::Float(b)), None) => {
                    let a = *a.value() as f32;
                    let b = *b.value() as f32;
                    Value::FloatPair(a, b)
                }
                (
                    Some(toml_edit::Value::Float(a)),
                    Some(toml_edit::Value::Float(b)),
                    Some(toml_edit::Value::Float(c)),
                ) => {
                    let a = *a.value() as f32;
                    let b = *b.value() as f32;
                    let c = *c.value() as f32;
                    Value::FloatRange(a, b, c)
                }
                _ => Value::Empty,
            },
            toml_edit::Value::InlineTable(table)
                if table
                    .get("_type")
                    .map(|t| t.as_str().map(|_t| _t == "complex").unwrap_or_default())
                    .unwrap_or_default() =>
            {
                let mut c = BTreeSet::new();
                for (key, _) in table.iter() {
                    c.insert(key.to_string());
                }
                crate::Value::Complex(c)
            }
            toml_edit::Value::InlineTable(blob) => match BlobInfo::try_from(blob) {
                Ok(blob_info) => match Value::try_from(&blob_info) {
                    Ok(v) => v,
                    Err(err) => {
                        error!("Could not convert blob info into value, {err}");
                        Value::Empty
                    }
                },
                Err(err) => {
                    error!("Could not convert table into blob info, {err}");
                    Value::Empty
                }
            },
        }
    }
}

impl TryFrom<&toml_edit::InlineTable> for super::blob::BlobInfo {
    type Error = Error;

    fn try_from(value: &toml_edit::InlineTable) -> Result<Self, Self::Error> {
        match (
            value
                .get("src")
                .and_then(|e| e.as_str())
                .map(|e| e.to_string()),
            value
                .get("data")
                .and_then(|d| d.as_str())
                .map(|d| d.to_string()),
        ) {
            (Some(src), Some(data)) => Ok(super::blob::BlobInfo {
                src,
                data,
                fetcher: None,
                ident: None,
            }),
            _ => Err("Could not create blob info from table".into()),
        }
    }
}

impl Build for TomlProperties {
    fn build(&self, lazy_builder: specs::world::LazyBuilder) -> Result<specs::Entity, Error> {
        let entity = lazy_builder.entity;

        let doc = self.clone();
        lazy_builder.lazy.exec_mut(move |world| {
            let build_log = {
                let mut toml_importer = TomlImporter {
                    world,
                    build_log: BuildLog::default(),
                    importing: None,
                };
                toml_importer.visit_document(&doc.doc);
                toml_importer.build_log
            };

            let result = world.write_component().insert(entity, build_log);
            trace!("Importing toml doc, result: {:?}", result);
        });

        Ok(lazy_builder.build())
    }
}

/// Struct for importing a toml build doc into World storage,
///
struct TomlImporter<'a> {
    world: &'a mut World,
    build_log: BuildLog,
    importing: Option<Importing>,
}

impl AsRef<World> for TomlImporter<'_> {
    fn as_ref(&self) -> &World {
        &self.world
    }
}

impl AsMut<World> for TomlImporter<'_> {
    fn as_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl crate::v2::compiler::WorldRef for TomlImporter<'_> {}

enum Importing {
    Block(Identifier),
    Root(Identifier),
}

impl<'a> toml_edit::visit::Visit<'a> for TomlImporter<'_> {
    fn visit_document(&mut self, node: &'a Document) {
        if let Some(blocks) = node["block"].as_table() {
            for (id, block) in blocks.iter() {
                if let (Some(ident), Some(block)) =
                    (id.parse::<Identifier>().ok(), block.as_table())
                {
                    self.importing = Some(Importing::Block(ident));
                    self.visit_table(block);
                }
            }
        }

        if let Some(roots) = node["root"].as_table() {
            for (id, root) in roots.iter() {
                if let (Some(ident), Some(root)) = (id.parse::<Identifier>().ok(), root.as_table())
                {
                    self.importing = Some(Importing::Root(ident));
                    self.visit_table(root);
                }
            }
        }

        if let Some(properties) = node["properties"].as_table() {
            // Use the current build log
            let build_log = self.build_log.clone();

            for (id, properties) in properties.iter() {
                if let (Some(ident), Some(properties)) =
                    (id.parse::<Identifier>().ok(), properties.as_table())
                {
                    let mut _properties = Properties::new(ident.clone());
                    for (k, v) in properties.iter() {
                        if let Some(value) = v.as_value().map(|i| Into::<Property>::into(i)) {
                            _properties.set(k, value);
                        }
                    }

                    build_log
                        .find_ref::<Identifier>(ident, self)
                        .map(|build_ref| {
                            let _ = build_ref.map(|_| Ok(_properties)).result().map_err(|e| {
                                error!("Could not map properties {e}");
                            });
                        });
                }
            }
        }
    }

    fn visit_table(&mut self, node: &'a Table) {
        match self.importing.take() {
            Some(importing) => match importing {
                Importing::Block(ident) => {
                    if let Some(roots) = node["roots"].as_array() {
                        let mut block = Block::new(ident.clone());

                        for _root in roots
                            .iter()
                            .filter_map(|i| i.as_str())
                            .filter_map(|i| i.parse::<Identifier>().ok())
                        {
                            block.add_root(_root, Value::Empty);
                        }

                        let entity = self
                            .world
                            .create_entity()
                            .with(ident.clone())
                            .with(block)
                            .build();
                        self.build_log
                            .index_mut()
                            .insert(ident.commit().expect("should be able to commit"), entity);
                    }
                }
                Importing::Root(ident) => {
                    if let Some(extensions) = node["extensions"].as_array() {
                        let mut root = Root::new(ident.clone(), Value::Empty);

                        for ext in extensions
                            .iter()
                            .filter_map(|i| i.as_str())
                            .filter_map(|i| i.parse::<Identifier>().ok())
                            .filter_map(|i| i.commit().ok())
                        {
                            root = root.extend(&ext);

                            let ext_entity = self.world.create_entity().with(ext.clone()).build();
                            self.build_log.index_mut().insert(ext, ext_entity);
                        }

                        let entity = self
                            .world
                            .create_entity()
                            .with(ident.clone())
                            .with(root)
                            .build();
                        self.build_log
                            .index_mut()
                            .insert(ident.commit().expect("should be able to commit"), entity);
                    }
                }
            },
            None => {}
        }
    }
}

impl From<toml_edit::TomlError> for Error {
    fn from(value: toml_edit::TomlError) -> Self {
        format!("toml error {value}").into()
    }
}
