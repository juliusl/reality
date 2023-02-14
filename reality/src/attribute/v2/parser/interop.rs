use specs::{Component, Entity, VecStorage};
use tracing::trace;

use crate::{v2::{Error, Action}, Keywords, Value};

/// Struct to interop handling custom attributes between the v1 parser and the v2 parser,
///
#[allow(dead_code)]
#[derive(Clone, Component, Debug)]
#[storage(VecStorage)]
pub struct Packet {
    /// Root name of the attribute unless it differs from ident, in which case it is the tag
    ///
    pub(crate) name: Option<String>,
    /// Property ident of this packet,
    ///
    pub(crate) property: Option<String>,
    /// Custom attribute identifier,
    ///
    pub(crate) ident: String,
    /// Keyword that parsed this attribute,
    ///
    pub(crate) keyword: Option<Keywords>,
    /// Entity that owns this attribute, likely a block,
    ///
    pub(crate) entity: Option<Entity>,
    /// Input (symbol) value intended for this attribute,
    ///
    pub(crate) input: String,
    /// Block namespace,
    ///
    pub(crate) block_namespace: String,
    /// Extension namespace,
    ///
    pub(crate) extension_namespace: String,
    /// Returns the line count this was parsed at, relative to the extension namespace,
    /// 
    pub(crate) line_count: usize,
    /// List of actions to apply w/ this packet,
    /// 
    pub(crate) actions: Vec<Action>,
}

impl Packet {
    /// Designated tag,
    /// 
    pub fn tag(&self) -> Option<&String> {
        self.name.as_ref().filter(|n| *n != &self.ident)
    }

    /// Returns the qualified extension ident,
    /// 
    /// Used to link extension implementation,
    /// 
    pub fn qualified_ext_ident(&self) -> String {
        format!(
            "{}.{}",self.extension_namespace, self.ident
        )
    }

    /// Returns the table ident that stores data related to this packet,
    /// 
    pub fn table_ident(&self) -> String {
        match self.name.as_ref() {
            Some(tag) if tag != &self.ident => {
                format!("{}.{}.{}.{}", self.block_namespace, tag, self.extension_namespace, self.ident)
            }
            _ => {
                format!("{}.{}.{}", self.block_namespace, self.extension_namespace, self.ident)
            }
        }
    }

    /// Returns input as a value,
    /// 
    pub fn input(&self) -> Value {
        Value::Symbol(
            self
                .input
                .trim_start_matches(&self.ident)
                .trim()
                .to_string(),
        )
    }
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
