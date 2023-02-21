use std::sync::Arc;

use specs::Builder;
use specs::Component;
use specs::VecStorage;
use tracing::trace;

use crate::v2::action;
use crate::v2::Action;
use crate::v2::Build;
use crate::AttributeParser;
use crate::BlockProperties;
use crate::Error;
use crate::Identifier;
use crate::Keywords;
use crate::Value;

/// Struct to interop handling custom attributes between the v1 parser and the v2 parser,
///
#[allow(dead_code)]
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Packet {
    /// Block identifier this packet is intended for,
    ///
    pub(crate) block_identifier: Identifier,
    /// Packet identifier,
    ///
    pub(crate) identifier: Identifier,
    /// Keyword that parsed this attribute,
    ///
    pub(crate) keyword: Keywords,
    /// List of actions to apply w/ this packet,
    ///
    pub(crate) actions: Vec<Action>,
}

impl Packet {
    /// Returns true if this is an add packet,
    ///
    pub fn is_add(&self) -> bool {
        self.keyword == Keywords::Add
    }

    /// Returns true if this is an extension packet,
    ///
    pub fn is_extension(&self) -> bool {
        self.keyword == Keywords::Extension
    }

    /// Returns true if this is a define packet,
    ///
    pub fn is_define(&self) -> bool {
        self.keyword == Keywords::Define
    }

    /// Merges packet, if merge conditions are not met,
    /// returns the packet as an Err(Packet)
    ///
    /// A packet is merged if it is a defined packet w/ the same block identifier and identifier
    ///
    pub fn merge_packet(&mut self, mut other: Packet) -> Result<(), MergeRejectedReason> {
        if other.is_extension() {
            Err(MergeRejectedReason::NewExtension(other))
        } else if other.is_add() {
            Err(MergeRejectedReason::NewRoot(other))
        } else if self.block_identifier != other.block_identifier {
            Err(MergeRejectedReason::DifferentBlockNamespace(other))
        } else if self.identifier != other.identifier {
            Err(MergeRejectedReason::UnrelatedPacket(other))
        } else {
            debug_assert_eq!(
                self.identifier, other.identifier,
                "Packet should have been rejected before this point"
            );
            debug_assert!(
                other.is_define(),
                "Only define packets should be able to reach this point"
            );

            trace!("Merging packets, {:?}", other.actions);

            self.actions.append(&mut other.actions);

            Ok(())
        }
    }
}

/// Enumeration of merge errors,
///
pub enum MergeRejectedReason {
    /// If the packet has a different block namespace, it cannot be merged,
    ///
    DifferentBlockNamespace(Packet),
    /// If the packet is a new root, it cannot be merged
    ///
    NewRoot(Packet),
    /// If the packet being merged is a new extension, it cannot be merged
    ///
    NewExtension(Packet),
    /// If the packet's identifiers do not match, it cannot be merged
    ///
    UnrelatedPacket(Packet),
}

/// Implement to try and extract a packet,
///
pub trait MakePacket {
    /// Tries to return a packet,
    ///
    fn try_make_packet(&self) -> Result<Packet, Error>;
}

/// Trait to handle packets parsed by a v1 parser,
///
pub trait PacketHandler {
    /// Called on each packet once,
    ///
    fn on_packet(&mut self, packet: Packet) -> Result<(), Error>;
}

/// Default no-op implementation that traces packet output,
///
impl PacketHandler for () {
    fn on_packet(&mut self, p: Packet) -> Result<(), Error> {
        trace!("{:?}", p);
        Ok(())
    }
}

impl MakePacket for AttributeParser {
    fn try_make_packet(&self) -> Result<Packet, Error> {
        let block_identifier = self.block_identifier();
        let identifier = self.current_identifier();
        self.attr_ident().map(|a| trace!("Custom attr {a}"));
        match self.keyword().unwrap_or(&Keywords::Error) {
            Keywords::Add => {
                let mut packet = Packet {
                    block_identifier: block_identifier.clone(),
                    identifier: identifier.clone(),
                    keyword: Keywords::Add,
                    actions: vec![action::with(
                        format!("{}", self.attr_ident().cloned().unwrap_or_default()),
                        self.value().clone(),
                    )],
                };

                if let Some(name) = self.value().symbol() {
                    packet.identifier.join(name)?;
                }

                Ok(packet)
            }
            Keywords::Define => {
                let mut packet = Packet {
                    block_identifier: block_identifier.clone(),
                    identifier: identifier.clone(),
                    keyword: Keywords::Define,
                    actions: vec![],
                };

                if let (Some(attr_ident), Some(property), Some(value)) =
                    (self.attr_ident(), self.property(), self.edit_value())
                {
                    // Key-Value pattern
                    packet.actions.push(action::with(
                        format!("{attr_ident}"),
                        Value::Symbol(property.clone()),
                    ));
                    packet
                        .actions
                        .push(action::with(format!("{property}"), value.clone()));
                } else if let (Some(property), Some(value)) = (self.property(), self.edit_value()) {
                    // Property pattern
                    packet.actions.push(action::with(property, value.clone()));
                } else if let (Some(attr_ident), Some(value)) =
                    (self.attr_ident(), self.value().symbol())
                {
                    // Custom Attribute pattern
                    packet
                        .actions
                        .push(action::with(attr_ident, Value::Symbol(value)));
                }

                Ok(packet)
            }
            Keywords::Extension => {
                let mut packet = Packet {
                    block_identifier: block_identifier.clone(),
                    identifier: identifier.clone(),
                    keyword: Keywords::Extension,
                    actions: vec![],
                };

                if let Some(input) = self.edit_value().and_then(|v| v.symbol()) {
                    packet.identifier.join(input)?;
                }

                Ok(packet)
            }
            _ => Err("Could not make packet".into()),
        }
    }
}

impl Build for Packet {
    fn build(&self, lazy_builder: specs::world::LazyBuilder) -> Result<specs::Entity, Error> {
        match self.keyword {
            Keywords::Extension => {
                let mut properties = BlockProperties::new(self.identifier.to_string());
                for a in self.actions.iter() {
                    if let Action::With(name, value) = a {
                        properties.add(name, value.clone());
                    }
                }

                let mut ident = self.identifier.commit()?;
                ident.set_parent(Arc::new(self.block_identifier.clone()));
                let ident = ident.commit()?;

                Ok(lazy_builder.with(properties).with(ident).build())
            }
            _ => Err("not implemented".into()),
        }
    }
}
