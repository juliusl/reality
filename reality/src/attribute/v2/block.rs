use std::sync::Arc;

use crate::AttributeParser;
use crate::Value;
use specs::Component;
use specs::HashMapStorage;
use toml_edit::Document;
use toml_edit::Item;

use super::Action;
use super::Attribute;
use super::ValueProvider;

/// Struct representing a .runmd block,
///
#[derive(Component, Default)]
#[storage(HashMapStorage)]
pub struct Block {
    /// Internal toml document compiled from .runmd block,
    ///
    toml: Arc<Document>,
    /// Root attributes,
    ///
    attributes: Vec<Attribute>,
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Block")
            .field("attributes", &self.attributes)
            .finish()
    }
}

impl Block {
    /// Returns a new block from a toml document
    ///
    pub fn new(toml: Document) -> Self {
        let toml = Arc::new(toml);

        let mut block = Self {
            toml,
            attributes: vec![],
        };

        block.toml["attributes"].as_array_of_tables().map(|roots| {
            // Parse attributes from root
            for root in roots {
                let ident = root["ident"].as_str();
                let input = &root["value"].as_str().map(|s| {
                    let mut parser = AttributeParser::default();
                    let attr = parser.parse(s);
                    attr.value().clone()
                });

                let attr = Attribute::new(
                    ident.unwrap_or(""),
                    input.clone().unwrap_or(crate::Value::Empty),
                );

                block.attributes.push(attr);
            }
        });

        block
    }

    /// Returns an iterator over extensions this block requires,
    /// 
    pub fn requires(&self) -> impl Iterator<Item = &Action> {
        self.attributes.iter().flat_map(|a| a.requires())
    }

    /// Returns the last attribute,
    ///
    pub fn last_mut(&mut self) -> Option<&mut Attribute> {
        self.attributes.last_mut()
    }

    /// Adds an attribute to the block,
    /// 
    pub fn add_attribute(&mut self, ident: impl Into<String>, value: impl Into<Value>) {
        self.attributes.push(Attribute::new(ident, value));
    }

    /// Returns the block name,
    ///
    pub fn name(&self) -> Option<String> {
        self.toml["name"].as_str().map(|s| s.to_string())
    }

    /// Returns the block symbol,
    ///
    pub fn symbol(&self) -> Option<String> {
        self.toml["symbol"].as_str().map(|s| s.to_string())
    }
}

impl<'a> core::ops::Index<&'a str> for Block {
    type Output = Item;

    fn index(&self, index: &'a str) -> &Self::Output {
        &self.toml[index]
    }
}

impl ValueProvider<'_> for Block {}

#[allow(unused_imports)]
mod tests {
    use toml_edit::Document;

    use crate::Value;

    use super::Block;

    #[test]
    #[tracing_test::traced_test]
    fn test_block() {
        let doc = r#"
        name   = "test_block"
        symbol = "test"

        [[attributes]]
        ident   = "person"
        value   = ".symbol John"

        [[attributes]]
        ident   = "person"
        value   = ".symbol Jacob"
        "#;

        let doc = doc.parse::<Document>().expect("should be able to parse");

        let block = Block::new(doc);
        assert_eq!("person", block.attributes[0].ident);
        assert_eq!(Value::Symbol("John".to_string()), block.attributes[0].value);

        assert_eq!("person", block.attributes[1].ident);
        assert_eq!(
            Value::Symbol("Jacob".to_string()),
            block.attributes[1].value
        );

        assert_eq!(Some("test_block".to_string()), block.name());
        assert_eq!(Some("test".to_string()), block.symbol());
    }
}
