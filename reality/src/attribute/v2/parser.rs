use std::sync::Arc;
use specs::Builder;
use specs::Join;
use specs::World;
use specs::WorldExt;
use specs::Write;
use specs::WriteStorage;
use tracing::trace;

use self::interop::MakePacket;
use crate::Identifier;
use crate::parser::PropertyAttribute;
use crate::CustomAttribute;
use crate::Error;

mod interop;
pub use interop::Packet;
pub use interop::PacketHandler;

/// V2 block parser implementation,
///
#[derive(Default)]
pub struct Parser {
    /// V1 Parser,
    ///
    /// The V1 parser will not
    ///
    v1_parser: Option<crate::Parser>,
    /// Caches a packet,
    ///
    packet_cache: Option<Packet>,
    /// Current root identifier,
    /// 
    root_identifier: Option<Arc<Identifier>>,
    /// Current block identifier,
    /// 
    block_identifier: Identifier,
}

impl Parser {
    /// Returns a new parser,
    ///
    pub fn new() -> Self {
        let v1_parser = Some(Self::new_v1_parser(None));
        Parser {
            v1_parser,
            packet_cache: None,
            root_identifier: None,
            block_identifier: Default::default(),
        }
    }

    /// Parses content and routes interop packets,
    ///
    /// Returns self w/ a v1_parser and updated World if successful,
    /// 
    pub fn parse(
        mut self,
        content: impl Into<String>,
        packet_handler: &mut impl PacketHandler,
    ) -> Result<Self, Error> {
        if let Some(parser) = self.v1_parser.take() {
            // V1 Parser will emit packets
            let parser = parser.parse(content.into());
            let mut world = parser.commit();
            world.maintain();
            
            // Ensure an empty parser is added
            world.insert(Self::default());
            world.exec(|(mut p, mut parser): (WriteStorage<Packet>, Write<Self>)| {
                for mut _p in p.drain().join() {
                    parser.route_packet(_p, packet_handler)?; 
                }

                parser.route_cache(packet_handler)?;
                Ok::<(), Error>(())
            })?;

            // Ensure existing parser is removed
            if let Some(existing) = world.remove::<Self>() {
                debug_assert!(existing.packet_cache.is_none(), "Packet cache should be empty");
            }

            // Update v1_parser
            self.v1_parser = Some(Self::new_v1_parser(Some(world)));
            Ok(self)
        } else {
            Err("Trying to parse w/ an unintialized parser".into())
        }
    }

    /// Routes an incoming packet,
    ///
    /// If the packet is an extension packet, it will be cached so that subsequent packets can be merged if applicable,
    ///
    /// Otherwise, the packet is routed to the destination packet handler.
    ///
    /// When a packet cannot be merged, the cache is cleared and routed to the destination handler.
    ///
    fn route_packet(
        &mut self,
        mut incoming: Packet,
        dest: &mut impl PacketHandler,
    ) -> Result<(), Error> {
        // Keep track of current block_identifier
        if self.block_identifier != incoming.block_identifier {
            self.block_identifier = incoming.block_identifier.clone();
            self.root_identifier.take();
        }

        if let Some(root_id) = self.root_identifier.as_ref().filter(|_| !incoming.is_add()) {
            incoming.identifier.set_root(root_id.clone());
        }

        if let Some(cached) = self.packet_cache.as_mut() {
            // Merge errors signal that the cache should be routed,
            // The rejected packet will be re-routed
            if let Err(err) = cached.merge_packet(incoming) {
                match err {
                    interop::MergeRejectedReason::DifferentBlockNamespace(next)
                    | interop::MergeRejectedReason::NewRoot(next)
                    | interop::MergeRejectedReason::NewExtension(next) => {
                        self.route_cache(dest)?;
                        self.route_packet(next, dest)?;
                    }
                    interop::MergeRejectedReason::UnrelatedPacket(unrelated) => {
                        unreachable!("If a packet is rejected for this reason it indicates a packet creation error, {:?}", unrelated)
                    }
                }
            }

            Ok(())
        } else if incoming.is_extension() {
            trace!("Packet cache is being updated, {:#}", incoming.identifier);
            // Hold on to incoming extension packets
            self.packet_cache = Some(incoming);
            Ok(())
        } else {
            if incoming.is_add() {
                self.root_identifier = Some(Arc::new(incoming.identifier.clone()));
            }

            dest.on_packet(incoming)
        }
    }

    /// Clears and routes the cached packet to the destination packet handler,
    ///
    fn route_cache(&mut self, dest: &mut impl PacketHandler) -> Result<(), Error> {
        if let Some(last) = self.packet_cache.take() {
            trace!(
                "Cached packet is being routed to dest, {:#}",
                last.identifier
            );
            dest.on_packet(last)?;
        }

        Ok(())
    }

    /// Returns a new v1 parser that will emit interop packets on custom attributes,
    ///
    fn new_v1_parser(world: Option<World>) -> crate::Parser {
        let mut v1_parser = world
            .map(|w| crate::Parser::new_with(w))
            .unwrap_or(crate::Parser::new());

        v1_parser.set_default_custom_attribute(CustomAttribute::new_with("", |parser, _| {
            if let Ok(packet) = parser.try_make_packet() {
                parser.lazy_exec_mut(move |w| {
                    w.register::<Packet>();
                    w.create_entity().with(packet).build();
                });
            }
        }));

        v1_parser.set_default_property_attribute(PropertyAttribute(|parser| {
            if let Ok(packet) = parser.try_make_packet() {
                parser.lazy_exec_mut(move |w| {
                    w.register::<Packet>();
                    w.create_entity().with(packet).build();
                });
            }
        }));
        v1_parser
    }
}

#[allow(unused_imports)]
mod tests {
    use super::Parser;
    use crate::v2::BlockList;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_parser() {
        let parser = Parser::new();
        let mut compiler = BlockList::default();
        let _parser = parser.parse(
            r#"
            ``` b block
            : test .true

            + test:v1 .op add
            : lhs .int
            : rhs .int
            : sum .int
            <> .input lhs : .type stdin
            <test> .input rhs
            <> .eval  sum
            ```

            ``` a block
            + test:v2 .op sub
            : lhs .int
            : rhs .int
            : diff .int
            <> .input lhs : .type stdin
            : count .int 1
            : test .env /host
            <test> .input rhs
            <> debug .eval diff

            + .op mult
            : lhs .int
            : rhs .int
            : prod .int
            <> .input lhs
            <> .input rhs
            <> .eval prod
            ```

            ``` block
            : root_test .true
            ```
        "#,
            &mut compiler,
        );

        for (_, b) in compiler.blocks() {
            println!("{}", b);
        }
    }
}
