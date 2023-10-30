use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde::de::Visitor;
use serde::Deserialize;
use serde::Serialize;

/// Field access,
///
#[derive(Debug)]
pub struct Field<'a, T> {
    /// Field owner type name,
    ///
    pub owner: &'static str,
    /// Name of the field,
    ///
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    ///
    pub value: &'a T,
}

/// Mutable field access,
///
#[derive(Debug)]
pub struct FieldMut<'a, T> {
    /// Field owner type name,
    ///
    pub owner: &'static str,
    /// Name of the field,
    ///
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Mutable access to the field,
    ///
    pub value: &'a mut T,
}

/// Field /w owned value,
///
#[derive(Debug)]
pub struct FieldOwned<T> {
    /// Field owner type name,
    ///
    pub owner: &'static str,
    /// Name of the field,
    ///
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    ///
    pub value: T,
}

/// Struct for containing an object safe Field representation,
///
#[derive(Serialize, Deserialize)]
pub struct FieldPacket {
    /// Pointer to data this packet has access to,
    ///
    #[serde(skip)]
    pub data: Option<Box<dyn FieldPacketType>>,
    /// Operation
    ///
    pub op_code: OpCode,
    /// Optional, wire data that can be used to create the field packet type,
    ///
    pub wire_data: Option<Vec<u8>>,
    /// Name of the type of data included
    ///
    pub data_type_name: &'static str,
    /// Size of the type of data,
    ///
    pub data_type_size: usize,
    /// Field offset in the owning type,
    ///
    pub field_offset: usize,
    /// Attribute hash value,
    ///
    pub attribute_hash: Option<u64>,
}

impl Default for OpCode {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl Serialize for OpCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(self.bits() as u32)
    }
}

impl<'de> Deserialize<'de> for OpCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_u32(OpCode::empty())
    }
}

impl<'de> Visitor<'de> for OpCode {
    type Value = OpCode;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(formatter, "Expecting unsigned integer")
    }

    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(OpCode::from_bits_truncate(v as usize))
    }
}

bitflags::bitflags! {
    /// Op code bit flags,
    ///
    #[derive(Clone, Copy)]
    pub struct OpCode : usize {
        /// Read the value of a field and set data,
        ///
        const READ = 1;
        /// Write the value of a from data,
        ///
        const WRITE = 1 << 1;
        /// Use wire_data instead of data,
        ///
        const WIRE = 1 << 2;
        /// Read the value of a field and set wire_data
        ///
        const READ_WIRE = OpCode::WIRE.bits() | OpCode::READ.bits();
        /// Write the value of a field to wire_data
        ///
        const WRITE_WIRE = OpCode::WIRE.bits() | OpCode::WRITE.bits();
    }
}

impl FieldPacket {
    /// Creates a new packet w/o data,
    ///
    pub fn new<T>() -> Self
    where
        T: FieldPacketType,
    {
        Self {
            op_code: OpCode::from_bits_truncate(0),
            wire_data: None,
            data: None,
            data_type_name: std::any::type_name::<T>(),
            data_type_size: std::mem::size_of::<T>(),
            field_offset: 0,
            attribute_hash: None,
        }
    }

    /// Creates a new packet w/ data to write a field with,
    ///
    pub fn new_data<T>(data: T) -> Self
    where
        T: FieldPacketType,
    {
        let mut packet = Self::new::<T>();
        if packet.data_type_name == std::any::type_name::<T>()
            && packet.data_type_size == std::mem::size_of::<T>()
        {
            packet.data = Some(Box::new(data));
            packet
        } else {
            packet
        }
    }

    /// Converts a field packet ptr into data,
    ///
    pub fn into_box<T>(self) -> Option<Box<T>>
    where
        T: Send + Sync + 'static,
    {
        if self.data_type_name != std::any::type_name::<T>()
            || self.data_type_size != std::mem::size_of::<T>()
        {
            return None;
        }
        /// Convert a mut borrow to a raw mut pointer
        ///
        fn from_ref_mut<T: ?Sized>(r: &mut T) -> *mut T {
            r
        }
        self.data.and_then(|t| {
            let t = Box::leak(t);

            let t = from_ref_mut(t);
            let t = t.cast::<T>();
            if !t.is_null() {
                Some(unsafe { Box::from_raw(t) })
            } else {
                None
            }
        })
    }

    /// Sets the op_code to Write,
    ///
    pub fn write(mut self) -> Self {
        self.op_code = OpCode::WRITE;
        self
    }

    /// Sets the op_code to Read,
    ///
    pub fn read(mut self) -> Self {
        self.op_code = OpCode::READ;
        self
    }

    /// Converts packet into wire mode,
    ///
    pub fn into_wire<T>(self) -> FieldPacket
    where
        T: FieldPacketType + Sized + Serialize + DeserializeOwned,
    {
        let mut packet = FieldPacket {
            op_code: self.op_code | OpCode::WIRE,
            data: None,
            data_type_name: std::any::type_name::<T>(),
            data_type_size: std::mem::size_of::<T>(),
            field_offset: self.field_offset,
            attribute_hash: self.attribute_hash,
            wire_data: None,
        };

        packet.wire_data = self.into_box::<T>().and_then(|d| d.to_binary().ok());
        packet
    }

    /// Sets the routing information for this packet,
    ///
    pub fn route(mut self, field_offset: usize, attribute: Option<u64>) -> Self {
        self.field_offset = field_offset;
        self.attribute_hash = attribute;
        self
    }
}

/// Implemented by a type that can be stored into a packet,
///
pub trait FieldPacketType: Send + Sync + 'static {
    /// Type that can be serialized to/from a string,
    ///
    fn from_str(str: &str, dest: &mut Option<Self>) -> anyhow::Result<()>
    where
        Self: FromStr + Sized;

    /// Type that can be deserialized to/from binary,
    ///
    fn from_binary(vec: Vec<u8>, dest: &mut Option<Self>) -> anyhow::Result<()>
    where
        Self: Serialize + DeserializeOwned,
    {
        let data = bincode::deserialize(&vec)?;
        let _ = dest.insert(data);
        Ok(())
    }

    /// Converts type to bincode bytes,
    ///
    fn to_binary(&self) -> anyhow::Result<Vec<u8>>
    where
        Self: Serialize + DeserializeOwned,
    {
        let ser = bincode::serialize(&self)?;
        Ok(ser)
    }
}

/// Trait for visiting fields w/ read-only access,
///
pub trait Visit<T> {
    /// Returns a vector of fields,
    ///
    fn visit<'a>(&'a self) -> Vec<Field<'a, T>>;
}

/// Trait for visiting fields w/ mutable access,
///
pub trait VisitMut<T> {
    /// Returns a vector of fields w/ mutable access,
    ///
    fn visit_mut<'a: 'b, 'b>(&'a mut self) -> Vec<FieldMut<'b, T>>;
}

/// Trait for setting a field,
///
pub trait SetField<T> {
    /// Sets a field on the receiver,
    ///
    /// Returns true if successful.
    ///
    fn set_field(&mut self, field: FieldOwned<T>) -> bool;
}

impl<T> FieldPacketType for T
where
    T: FromStr + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn from_str(str: &str, dest: &mut Option<Self>) -> anyhow::Result<()>
    where
        Self: FromStr + Sized,
    {
        if let Ok(value) = <T as FromStr>::from_str(str) {
            let _ = dest.insert(value);
        }
        Ok(())
    }
}

mod tests {
    use super::FieldMut;
    use crate::prelude::*;

    pub mod reality {
        pub use crate::*;
        pub mod prelude {
            pub use crate::prelude::*;
        }
    }

    #[derive(Reality, Default)]
    struct Test {
        #[reality(derive_fromstr)]
        name: String,
        other: String,
    }

    #[test]
    fn test_visit() {
        let mut test = Test {
            name: String::from(""),
            other: String::new(),
        };
        {
            let mut fields = test.visit_mut();
            let mut fields = fields.drain(..);
            if let Some(FieldMut { name, value, .. }) = fields.next() {
                assert_eq!("name", name);
                *value = String::from("hello-world");
            }

            if let Some(FieldMut { name, value, .. }) = fields.next() {
                assert_eq!("other", name);
                *value = String::from("hello-world-2");
            }
        }
        assert_eq!("hello-world", test.name.as_str());
        assert_eq!("hello-world-2", test.other.as_str());
    }

    #[test]
    fn test_packet() {
        let packet = crate::attributes::visit::FieldPacket::new_data(String::from("Hello World"));
        let packet = packet.into_box::<String>();
        let packet_data = packet.expect("should be able to convert");
        let packet_data = packet_data.as_str();
        assert_eq!("Hello World", packet_data);

        let packet = crate::attributes::visit::FieldPacket::new_data(String::from("Hello World"));
        let packet = packet.into_box::<Vec<u8>>();
        assert!(packet.is_none());

        let packet = crate::attributes::visit::FieldPacket::new_data(String::from("Hello World"));
        let packet = packet
            .write()
            .route(0, None)
            .into_wire::<String>();
        println!("{:?}", packet.wire_data);
    }
}
