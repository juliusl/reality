use std::collections::BTreeMap;
use tracing::trace;

use crate::Error;
use crate::Identifier;
use crate::Keywords;

use super::action;
use super::parser::Packet;
use super::Block;

/// Struct for mapping runmd blocks,
///  
#[derive(Default)]
pub struct BlockList {
    /// List of blocks,
    ///
    blocks: BTreeMap<Identifier, Block>,
}

impl BlockList {
    /// Returns the current block map,
    ///
    pub fn blocks(&self) -> &BTreeMap<Identifier, Block> {
        &self.blocks
    }
}

impl super::parser::PacketHandler for BlockList {
    fn on_packet(&mut self, packet: super::parser::Packet) -> Result<(), Error> {
        trace!(
            "\n\tblock: {:#}\n\tident: {:#}\n\tkeyword: {:?}\n\tactions {:?}",
            packet.block_identifier,
            packet.identifier,
            packet.keyword,
            packet.actions
        );
        if !self.blocks.contains_key(&packet.block_identifier) {
            self.blocks.insert(
                packet.block_identifier.clone(),
                Block::new(packet.block_identifier.clone()),
            );
        }

        if let Some(block) = self.blocks.get_mut(&packet.block_identifier) {
            match packet {
                Packet {
                    keyword: Keywords::Add,
                    identifier,
                    actions,
                    ..
                } if !actions.is_empty() => {
                    if let Some(action::Action::With(_, value)) = actions.first() {
                        block.add_attribute(identifier, value.clone());
                    }
                }
                Packet {
                    keyword: Keywords::Define,
                    mut actions,
                    ..
                } if !actions.is_empty() && block.attribute_count() == 0 => {
                    for a in actions.drain(..) {
                        block.initialize_with(a);
                    }
                }
                Packet {
                    keyword: Keywords::Define,
                    mut actions,
                    ..
                } if !actions.is_empty() && block.attribute_count() > 0 => {
                    block.last_mut().map(|l| {
                        for a in actions.drain(..) {
                            l.push(a);
                        }
                    });
                }
                Packet {
                    keyword: Keywords::Extension,
                    identifier,
                    ..
                } => {
                    block.last_mut().map(|l| {
                        *l = l.clone().extend(&identifier);
                    });

                    // let identifier = identifier.commit()?;

                    // for a in actions {
                    //     match a {
                    //         action::Action::With(name, value) => {
                    //             let ident = identifier.branch(name)?;
                    //             block.last_mut().map(|l| {
                    //                 *l = l.clone().with(ident.to_string(), value);
                    //             });
                    //         }
                    //         _ => {}
                    //     }
                    // }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[allow(unused_imports)]
mod tests {
    use toml_edit::{table, value, visit::Visit, ArrayOfTables, Document, Item, Table};

    use crate::Identifier;

    #[test]
    fn visitor_prototype() {
        let example = r#"
[a.block."block"]
_e = true 

[[a.block."block".attributes]]
[a.block."block".attributes.op.sub."test:v2"]
_e = true 

[[a.block."block".attributes.op.sub."test:v2".extensions]]
[a.block."block".attributes.op.sub."test:v2".extensions.input.lhs]
_e = true 

[[a.block."block".attributes.op.sub."test:v2".extensions]]
[a.block."block".attributes.op.sub."test:v2".extensions.test.input.rhs]
_e = true 

[[a.block."block".attributes.op.sub."test:v2".extensions]]
[a.block."block".attributes.op.sub."test:v2".extensions.eval.diff."debug"]
_e = true 
        
[[a.block."block".attributes]]
[a.block."block".attributes.op.mult]
_e = true 

[[a.block."block".attributes.op.mult.extensions]]
[a.block."block".attributes.op.mult.extensions.input.lhs]
_e = true 

[[a.block."block".attributes.op.mult.extensions]]
[a.block."block".attributes.op.mult.extensions.input.rhs]
_e = true 

[[a.block."block".attributes.op.mult.extensions]]
[a.block."block".attributes.op.mult.extensions.eval.prod]
_e = true 
        
        
[b.block."block"]
_e = true 

[[b.block."block".attributes]]
[b.block."block".attributes.op.add."test:v1"]
_e = true 

[[b.block."block".attributes.op.add."test:v1".extensions]]
[b.block."block".attributes.op.add."test:v1".extensions.input.lhs]
_e = true 

[[b.block."block".attributes.op.add."test:v1".extensions]]
[b.block."block".attributes.op.add."test:v1".extensions.test.input.rhs]
_e = true 

[[b.block."block".attributes.op.add."test:v1".extensions]]
[b.block."block".attributes.op.add."test:v1".extensions.eval.sum]
_e = true
"#;

        let mut example = example.parse::<Document>().unwrap();
        // println!("{}", example["a"]["test"]["block"]);
        // println!("{}", example["a"]["test"]["block"]["op"]["add"]["extensions"].as_array_of_tables().unwrap());

        let mut test = Test::default();
        test.current = "v1".parse().expect("should parse");
        test.visit_document(&example);

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
