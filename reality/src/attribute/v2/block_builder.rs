use std::collections::BTreeMap;

use toml_edit::{Document, Table};

use crate::Value;

use super::Block;

/// Struct for transpiling runmd interop into a TOML document,
///  
#[derive(Default)]
pub struct BlockBuilder {
    /// Compiled blocks,
    ///
    blocks: BTreeMap<String, Block>,
}

impl BlockBuilder {
    /// Returns the current block map,
    ///
    pub fn blocks(&self) -> &BTreeMap<String, Block> {
        &self.blocks
    }
}

impl super::parser::PacketHandler for BlockBuilder {
    fn on_packet(&mut self, packet: super::parser::Packet) -> Result<(), super::Error> {
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
                        block.add_attribute(ident, value);

                        packet.tag().as_ref().map(|tag| {
                            block
                                .last_mut()
                                .map(|b| b.set_tag(*tag));
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
                        block.last_mut().map(|a| {
                            *a = a.clone()
                                .with(ident, value)
                                .extend(ext_ident);
                        });
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
