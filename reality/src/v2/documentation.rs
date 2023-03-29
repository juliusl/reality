use super::Action;
use super::parser::PacketHandler;

/// Struct for storing documentation,
/// 
#[derive(Default)]
pub struct Documentation;

impl PacketHandler for Documentation {
    fn on_packet(&mut self, packet: super::parser::Packet) -> Result<(), crate::Error> {
        if packet.actions.iter().any(|a| if let Action::Doc(_) = a { true } else { false }) {
            println!("Found doc -- \n\t{:#}\n\t{:?}", packet.identifier, packet.keyword);
            for a in packet.actions.iter() {
                println!("\t{:?}", a);
            }
        }

        Ok(())
    }
}