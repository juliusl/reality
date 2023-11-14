use std::str::FromStr;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use tracing::error;

use crate::Attribute;
use crate::ResourceKey;

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
    pub owner: String,
    /// Name of the field,
    ///
    pub name: String,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    ///
    pub value: T,
}

/// Type-alias for a the frame version of an attribute type,
///
pub type Frame = Vec<FieldPacket>;

/// Wrapper struct over frames meant to update a block object,
/// 
#[derive(Clone, Default, Debug)]
pub struct FrameUpdates(pub Frame);

/// Converts a type to a list of packets,
///
pub trait ToFrame {
    /// Returns the current type as a Frame,
    ///
    fn to_frame(&self, key: Option<ResourceKey<Attribute>>) -> Frame;
}

/// Struct for containing an object safe Field representation,
///
#[derive(Serialize, Deserialize)]
pub struct FieldPacket {
    /// Pointer to data this packet has access to,
    ///
    #[serde(skip)]
    pub data: Option<Box<dyn FieldPacketType>>,
    /// Optional, wire data that can be used to create the field packet type,
    ///
    pub wire_data: Option<Vec<u8>>,
    /// Name of the type of data included
    ///
    pub data_type_name: String,
    /// Size of the type of data,
    ///
    pub data_type_size: usize,
    /// Field offset in the owning type,
    ///
    pub field_offset: usize,
    /// Name of the field,
    ///
    pub field_name: String,
    /// Type name of the owner of this field,
    ///
    pub owner_name: String,
    /// Attribute hash value,
    ///
    pub attribute_hash: Option<u64>,
    /// Operation code,
    /// (TODO)
    #[serde(skip)]
    op: u128,
}

impl Clone for FieldPacket {
    fn clone(&self) -> Self {
        Self {
            data: None,
            wire_data: self.wire_data.clone(),
            data_type_name: self.data_type_name.clone(),
            data_type_size: self.data_type_size,
            field_offset: self.field_offset,
            field_name: self.field_name.clone(),
            owner_name: self.owner_name.clone(),
            attribute_hash: self.attribute_hash,
            op: self.op,
        }
    }
}

impl std::fmt::Debug for FieldPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldPacket")
            .field("wire_data", &self.wire_data)
            .field("data_type_name", &self.data_type_name)
            .field("data_type_size", &self.data_type_size)
            .field("field_offset", &self.field_offset)
            .field("field_name", &self.field_name)
            .field("owner_name", &self.owner_name)
            .field("attribute_hash", &self.attribute_hash)
            .field("op", &self.op)
            .finish()
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
            wire_data: None,
            data: None,
            data_type_name: std::any::type_name::<T>().to_string(),
            data_type_size: std::mem::size_of::<T>(),
            field_name: String::new(),
            owner_name: String::new(),
            field_offset: 0,
            attribute_hash: None,
            op: 0,
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
        T: DeserializeOwned + Send + Sync + 'static,
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

        if self.data.is_none() {
            if let Some(wire) = self.wire_data {
                return if let Ok(decoded) = bincode::deserialize(&wire) {
                    Some(Box::new(decoded))
                } else {
                    error!("Could not deserialize encoded value");
                    None
                };
            } else {
                error!("Field packet is completely empty");
                return None;
            }
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

    /// Converts packet into wire mode,
    ///
    pub fn into_wire<T>(self) -> FieldPacket
    where
        T: FieldPacketType + Sized + Serialize + DeserializeOwned,
    {
        let mut packet = FieldPacket {
            data: None,
            data_type_name: std::any::type_name::<T>().to_string(),
            data_type_size: std::mem::size_of::<T>(),
            field_offset: self.field_offset,
            field_name: self.field_name.to_string(),
            attribute_hash: self.attribute_hash,
            wire_data: None,
            owner_name: self.owner_name.to_string(),
            op: 0,
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

    /// Convert the packet into an owned field,
    ///
    pub fn into_field_owned(self) -> FieldOwned<FieldPacket> {
        FieldOwned {
            owner: self.owner_name.clone(),
            name: self.field_name.clone(),
            offset: self.field_offset,
            value: self,
        }
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
    fn visit(&self) -> Vec<Field<'_, T>>;
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
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
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

#[allow(unused_imports)]
mod tests {
    use super::FieldMut;
    use crate::prelude::*;

    pub mod reality {
        pub use crate::*;
        pub mod prelude {
            pub use crate::prelude::*;
        }
    }

    use async_trait::async_trait;

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
        let packet = packet.route(0, None).into_wire::<String>();
        println!("{:?}", packet.wire_data);
    }
}
