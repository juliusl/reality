use std::collections::BTreeMap;
use crate::Error;
use crate::Identifier;

use super::Block;
use super::action;

/// Struct for mapping runmd blocks,
///  
#[derive(Default)]
pub struct BlockList {
    /// List of blocks,
    ///
    blocks: BTreeMap<String, Block>,
}

impl BlockList {
    /// Returns the current block map,
    ///
    pub fn blocks(&self) -> &BTreeMap<String, Block> {
        &self.blocks
    }
}

impl super::parser::PacketHandler for BlockList {
    fn on_packet(&mut self, packet: super::parser::Packet) -> Result<(), Error> {
        if !self.blocks.contains_key(&packet.block_namespace) {
            self.blocks
                .insert(packet.block_namespace.to_string(), Block::default());
        }

        let value = packet.input();
        let ident = packet.ident.to_string();
        let ext_ident = packet.qualified_ext_ident();

        if let Some(block) = self.blocks.get_mut(&packet.block_namespace) {
            match packet.keyword.as_ref() {
                Some(keyword) => match keyword {
                    crate::Keywords::Add => {
                        let mut ident: Identifier = ident.parse()?;

                        if let Some(symbol) = value.symbol() {
                            ident.join(symbol)?;
                        }

                        block.add_attribute(ident, value);

                        packet.tag().as_ref().map(|tag| {
                            block
                                .last_mut()
                                .map(|b| b.set_tags(*tag));
                        });

                        if !packet.actions.is_empty() {
                            block.last_mut().map(|b|{
                                let mut _b = b.clone();

                                for a in packet.actions.iter() {
                                    _b.push(a.clone()); 
                                }

                                *b = _b;
                            });
                        }
                    }
                    crate::Keywords::Define | crate::Keywords::Extension => {
                        if let None = block.last_mut().map(|a| {
                            *a = a.clone()
                                .extend(&ext_ident, value.clone());
                        }) {
                            block.initialize_with(action::with(ext_ident, value));
                        }
                    }
                    _ => {
                        unreachable!("Only keywords that emit packets would reach this code")
                    }
                },
                _ => {}
            }
        }

        Ok(())
    }
}

#[allow(unused_imports)]
mod tests{
    use toml_edit::{Document, Table, value, table, ArrayOfTables, Item, visit::Visit};

    use crate::Identifier;

    #[test]
    fn visitor_prototype() {
        let example = r#"
[a.test."block"]
_e = true

[[a.test."block".attributes]]
[a.test."block".attributes.op.add]
_e = true

[[a.test."block".attributes.op.add.extensions]]
[a.test."block".attributes.op.add.extensions.input.lhs]
_e = true

[[a.test."block".attributes.op.add.extensions]]
[a.test."block".attributes.op.add.extensions.input.rhs]
_e = true

[[a.test."block".attributes.op.add.extensions]]
[a.test."block".attributes.op.add.extensions.eval.sum]
_e = true
        "#;

        let mut example = example.parse::<Document>().unwrap();
        // println!("{}", example["a"]["test"]["block"]);
        // println!("{}", example["a"]["test"]["block"]["op"]["add"]["extensions"].as_array_of_tables().unwrap());
        
        let mut test = Test::default();
        test.visit_document(&example);

        example["a"]["test"]["block"]["attributes"][0]["op"]["add"]["extensions"].as_array_of_tables().unwrap();

        for id in test.stack {
            println!("{:#}", id)
        }

        example.fmt();
        println!("{}", example);
    }

    #[derive(Default)]
    struct Test {
        current: Identifier,
        stack: Vec<Identifier>,
    }

    impl<'doc> Visit<'doc> for Test {
        fn visit_table(&mut self, node: &'doc Table) {
            for (key, item) in node.iter() {
                if item.is_table() || item.is_array_of_tables() {
                    if self.current.join(key).is_ok() {
                        if item.get("_e").and_then(|e| e.as_bool()) == Some(true) {
                            if let Ok(next) = self.current.commit() {
                                self.stack.push(next);
                            }
                        }
    
                        self.visit_item(item);
                        if let Ok(next) = self.current.truncate(1) {
                            self.current = next;
                        }
                    }
                }
            }
        }
    }
}
