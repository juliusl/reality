use std::collections::BTreeMap;

use toml_edit::Document;

use crate::Value;

use super::Block;

/// Struct for transpiling runmd interop into a TOML document,
///  
#[derive(Default)]
pub struct TomlTranspiler {
    /// Compiled blocks,
    ///
    blocks: BTreeMap<String, Block>,
    /// Transpiler result
    /// 
    toml: Document
}

impl TomlTranspiler {
    /// Returns the current block map,
    ///
    pub fn blocks(&self) -> &BTreeMap<String, Block> {
        &self.blocks
    }
}

impl super::parser::PacketHandler for TomlTranspiler {
    fn on_packet(&mut self, packet: super::parser::Packet) -> Result<(), super::Error> {
        if !self.blocks.contains_key(&packet.block_namespace) {
            self.blocks
                .insert(packet.block_namespace.to_string(), Block::default());
        }

        let value = packet.input();
        let ident = packet.ident.to_string();
        let ext_ident = packet.qualified_ext_ident();

        if let Some(block) = self.blocks.get_mut(&packet.block_namespace) {
            match packet.keyword {
                Some(keyword) => match keyword {
                    crate::Keywords::Add => {
                        block.add_attribute(ident, value);

                        packet.name.filter(|n| n != &packet.ident).map(|s| {
                            block
                                .last_mut()
                                .map(|b| *b = b.clone().with("tag", Value::Symbol(s)));
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
                            *a = a.clone().with(ident, value).extend(ext_ident);
                        });
                    }
                    crate::Keywords::Comment
                    | crate::Keywords::Error
                    | crate::Keywords::BlockDelimitter
                    | crate::Keywords::NewLine => {
                        unreachable!("These keywords would never emit packets")
                    }
                },
                _ => {}
            }
        }

        Ok(())
    }
}
