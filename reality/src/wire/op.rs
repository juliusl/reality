//! # Design Memo: Wire protocol
//!
//! The intention of this protocol is not to be a first-class rpc channel between objects.
//! It shouldn't be used to drive any type of high-performance rpc scenarios since existing solutions exist
//! and offer more flexibility.
//!
//! Instead, it is a simple internal-level backhaul protocol that can be used to make tiny adjustments or fine
//! tuning. This should be considered very-low level and the average contributor will probably never interface
//! with this directly. Even so, since it might touch many places it's important that the design is documented
//! as thouroughly as possible.
//!
//! # Wire operation data layout
//!
//! A wire operation is a custom encoded uuid. A uuid consists of 4 parts,
//!
//! u32 u16 u16 [u8; 8]
//!
//! A uuid can also be viewed as 2 u64 values.
//!
//! This makes a uuid a convenient struct to build on top of as a data structure.
//!
//! # Wire operations
//!
//! If enabled, a wire operation is stored on the end of a field packet and can communicate
//! instructions for decoding and applying the field packet at the destination.
//!
//! Setting the wire operation is optional, the type of instructions communicated by it are only
//! optimizations and may or may not apply. Basically, these wire codes **must** be safe.
//!
//! The following are ideas of some optimizations that could be useful to communicate,
//!
//! **Inserting an element**, If communicated inserting an element to an offset could be enabled.
//!
//! By default, the offset requires the projected type. Which means that if the element type was set in the
//! packet addressed to the same offset it would be "corrected" to the projected type w/ 1 element.
//!
//! If enabled this could allow the element to be inserted instead of being allocated as the projected type of an
//! element.
//!
//! **Indexing a type**, If communicated, treat the incoming packet as informational and index the current values so
//! that subsequent packets are decoded more effeciently.
//!
//! **Reserving space**, If communicated, this could notify the receiver of the size of an incoming packet so that the receiver
//! can ensure there is enough space ready to receive the packet.
//!
//! **Health checks**, If communicated, could send over some diagnostic level data.
//!
//! # Initial design --
//!
//! - u32 -- Reserve, could be useful later
//!
//! - u16 -- Instruction bitflags
//! - u16 -- Mode bitflags (0..16)
//!
//! u16 x u16 possible combinations should be enough future space
//!
//! - [u8; 8] -- Data-bytes; 64 bits used as input for the operation. Usage determined by instruction handler.
//!

/// Op data,
///
#[derive(Copy, Hash, PartialEq, PartialOrd, Clone, Eq, Ord, Debug, Default)]
pub struct Op(uuid::Uuid);

impl Op {
    /// Returns the current instruction mode,
    ///
    pub fn _instruction(self) -> Instruction {
        let (_, i, _, _) = self.0.as_fields();

        Instruction::from_bits_truncate(i)
    }

    /// Returns the current operation mode,
    ///
    pub fn _mode(self) -> Mode {
        let (_, _, m, _) = self.0.as_fields();

        Mode::from_bits_truncate(m)
    }
}

bitflags::bitflags! {
    /// Instruction bit flags,
    ///
    pub struct Instruction: u16 {
        const Insert = 1;
        const Index = 1 << 1;
        const Reserve = 1 << 2;
        const Health = 1 << 3;
    }

    /// Mode bit flags,
    ///
    pub struct Mode: u16 {

    }
}
