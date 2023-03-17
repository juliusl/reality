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
                } if !actions.is_empty() && block.root_count() == 0 => {
                    for a in actions.drain(..) {
                        block.initialize_with(a);
                    }
                }
                Packet {
                    keyword: Keywords::Define,
                    mut actions,
                    ..
                } if !actions.is_empty() && block.root_count() > 0 => {
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
                }
                _ => {}
            }
        }
        Ok(())
    }
}
