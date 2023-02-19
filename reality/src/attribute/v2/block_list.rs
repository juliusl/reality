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

mod tests{
    use toml_edit::{Document, Table, value, table, ArrayOfTables, Item};

    #[test]
    fn test_dotted() {
        // let mut doc = Document::new();
        // // let mut table = Table::new();
        // // table.set_dotted(true);

        // doc["block"] = table();
        // doc["block"]["op"] = table();
        // doc["block"]["op"]["add"] = table();
        // doc["block"]["op"]["add"]["float"]["lhs"] = value(0.0);
        // doc["block"]["op"]["add"]["float"]["rhs"] = value(0.0);
        // doc["block"]["op"]["add"]["float"]["sum"] = value(0.0);
        // doc["block"]["op"]["add"]["float"].as_inline_table_mut().map(|t| t.set_dotted(true));

        // doc["block"]["op"]["add"]["float"]["input"]["value"] = value("lhs");
        // println!("{doc}");

        // println!("{}", doc["block"]["op"]["add"]["float"]);


        let example = r#"
[op]
add.lhs = 0
add.rhs = 0
add.sum = 0
add.actions = [
  { input = "lhs" },
  { input = "rhs" },
  { eval = "sum" }
]
        "#;

        let mut example = example.parse::<Document>().unwrap();
        println!("{}", example["op"]["add"]);
        println!("{}", example["op"]["add"]["actions"]);

        println!("{}", example);
    }
}
