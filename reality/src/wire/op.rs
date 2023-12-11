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

bitflags::bitflags! {
    /// Code-set, consists of 16-bit operations w/ 16-bit modes each.
    /// 
    /// Code-set maps to actual language verbs.
    /// 
    /// # Language Definition:
    /// 
    /// | op     |    mode    | description |
    /// | ------ | ---------- | ---------------------------- |
    /// | Op0    |            | Load operations              |
    /// | ------ | ---------- | ---------------------------- |
    /// | Op0    | Mode0      | Replace value                |
    /// | Op0    | Mode1      | Parse value                  |
    /// | Op0    | Mode2      | Merge value                  |
    /// | Op0    | Mode3      | Append value                 |
    /// | ------ | ---------- | ---------------------------- |
    /// | Op1    |            | Read operation               |
    /// | ------ | ---------- | ---------------------------- |
    /// | Op1    | Mode0      | bincode -> UTF-8 String      |
    /// | Op1    | Mode1      | bincode -> Decorated<String> |
    /// | ------ | ---------- | ---------------------------- |
    /// | Op2    |            | Util operations              |
    /// | ------ | ---------- | ---------------------------- |
    /// 
    /// 
    pub struct Code: u32 {
        const Op0 = 0;
        const Op1 = 1 << 1;
        const Op2 = 1 << 2;
        const Op3 = 1 << 3;
        const Op4 = 1 << 4;
        const Op5 = 1 << 5;
        const Op6 = 1 << 6;
        const Op7 = 1 << 7;
        const Op8 = 1 << 8;
        const Op9 = 1 << 9;
        const OpA = 1 << 10;
        const OpB = 1 << 11;
        const OpC = 1 << 12;
        const OpD = 1 << 13;
        const OpE = 1 << 14;
        const OpF = 1 << 15;
        const Mode0 = 1 << 16;
        const Mode1 = 1 << 17;
        const Mode2 = 1 << 18;
        const Mode3 = 1 << 19;
        const Mode4 = 1 << 20;
        const Mode5 = 1 << 21;
        const Mode6 = 1 << 22;
        const Mode7 = 1 << 23;
        const Mode8 = 1 << 24;
        const Mode9 = 1 << 25;
        const ModeA = 1 << 26;
        const ModeB = 1 << 27;
        const ModeC = 1 << 28;
        const ModeD = 1 << 29;
        const ModeE = 1 << 30;
        const ModeF = 1 << 31;

        /// Load value operation,
        /// 
        const Load = Self::Op0.bits();
        /// Load value by replacing existing value,
        /// 
        const Replace = Self::Load.bits() | Self::Mode0.bits();
        /// Load value by parsing input data as string input,
        /// 
        const Parse = Self::Load.bits() | Self::Mode1.bits();
        /// Load value by merging input data,
        /// 
        const Merge = Self::Load.bits() | Self::Mode2.bits();
        /// Load value by appending input data,
        /// 
        const Append = Self::Load.bits() | Self::Mode3.bits();

        /// Read value operation,
        /// 
        const Read = Self::Op1.bits();
        /// Read value as a bincode utf8 string,
        /// 
        const String = Self::Op1.bits() | Self::Mode0.bits();
        /// Read value as a bincode Decorated<String>,
        /// 
        const DecoratedString = Self::Op1.bits() | Self::Mode1.bits();

        /// Util operation,
        /// 
        const Util = Self::Op2.bits();
        
    }
}
