use std::sync::Arc;

use crate::v2::Visitor;
use crate::Value;
use specs::Component;
use specs::HashMapStorage;
use toml_edit::table;
use toml_edit::value;
use toml_edit::Array;
use toml_edit::Document;
use toml_edit::Item;

/// Struct for building a TOML-document from V2 compiler build,
///
#[derive(Default)]
pub struct DocumentBuilder {
    doc: Document,
    identifier: Option<String>,
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
}

impl Visitor for DocumentBuilder {
    fn visit_block(&mut self, block: &crate::v2::Block) {
        let owner = block.ident();
        let owner = owner.commit().unwrap().to_string().replace("\"", "");
        self.doc["block"][&owner] = table();

        let mut roots = Array::new();
        for root in block.roots() {
            roots.push(format!("{:#}", root.ident).replace("\"", ""));
            self.visit_root(root);
        }

        self.doc["block"][&owner].as_table_mut().map(|t| {
            t.set_implicit(true);
            t["roots"] = value(roots);
        });
    }

    fn visit_root(&mut self, root: &crate::v2::Root) {
        let owner = root.ident.clone();
        let owner = owner.commit().unwrap().to_string().replace("\"", "");
        self.doc["root"][&owner] = table();

        let mut extensions = Array::new();
        for ext in root.extensions() {
            extensions.push(format!("{:#}", ext).replace("\"", ""));
        }

        self.doc["root"][&owner].as_table_mut().map(|t| {
            t.set_implicit(true);
            t["extensions"] = value(extensions);
        });
    }

    fn visit_properties(&mut self, properties: &crate::v2::Properties) {
        let owner = properties.owner();
        let owner = owner.commit().unwrap().to_string().replace("\"", "");
        self.doc["properties"][&owner] = table();
        self.doc["properties"][&owner]
            .as_table_mut()
            .map(|t| t.set_implicit(true));
        self.identifier = Some(owner);

        for (name, property) in properties.iter_properties() {
            self.visit_property(name, property);
        }
    }

    fn visit_value(&mut self, name: &String, idx: Option<usize>, value: &Value) {
        self.identifier.as_ref().map(|id| {
            if let Some(0) = idx {
                self.doc["properties"][id][name] = toml_edit::value(Array::new());
                let item: Item = value.clone().into();

                if let Some(value) = item.as_value() {
                    self.doc["properties"][id][name]
                        .as_array_mut()
                        .map(|a| a.push(value));
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
            Value::BinaryVector(b) => value(base64::encode(b)),
            Value::Reference(r) => value(r as i64),
            Value::Symbol(s) => value(s),
            Value::Complex(_) => unimplemented!("not implemented"),
        }
    }
}

impl Into<Document> for DocumentBuilder {
    fn into(self) -> Document {
        self.doc
    }
}

impl Into<DocumentComponent> for DocumentBuilder {
    fn into(self) -> DocumentComponent {
        DocumentComponent {
            doc: Arc::new(self.into()),
        }
    }
}

/// Struct for component impl of toml document,
///
#[derive(Component, Clone)]
#[storage(HashMapStorage)]
pub struct DocumentComponent {
    pub doc: Arc<Document>,
}
