use std::str::FromStr;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing::error;

use crate::FieldOwned;

/// Implemented by a type that can be stored into a packet,
///
pub trait FieldPacketType: Send + Sync + 'static {
    /// Type that can be serialized to/from a string,
    ///
    fn from_str_to_dest(str: &str, dest: &mut Option<Self>) -> anyhow::Result<()>
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

impl<T> FieldPacketType for T
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    fn from_str_to_dest(str: &str, dest: &mut Option<Self>) -> anyhow::Result<()>
    where
        Self: FromStr + Sized,
    {
        if let Ok(value) = <T as FromStr>::from_str(str) {
            let _ = dest.insert(value);
        }
        Ok(())
    }
}

/// Struct for containing an object safe Field representation,
///
#[derive(Default, Serialize, Deserialize)]
pub struct FieldPacket {
    /// Pointer to data this packet has access to,
    ///
    #[serde(skip)]
    pub data: Option<Box<dyn FieldPacketType>>,
    /// Size of the type of data,
    ///
    pub data_type_size: usize,
    /// Field offset in the owning type,
    ///
    pub field_offset: usize,
    /// Name of the type of data included
    ///
    pub data_type_name: String,
    /// Name of the field,
    ///
    pub field_name: String,
    /// Type name of the owner of this field,
    ///
    pub owner_name: String,
    /// Operation code,
    ///
    #[serde(skip)]
    pub(crate) op: u128,
    /// Attribute hash value,
    ///
    pub attribute_hash: Option<u128>,
    /// Optional, wire data that can be used to create the field packet type,
    ///
    pub wire_data: Option<Vec<u8>>,
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
    pub fn new<T>() -> Self {
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
            // SAFETY: This is to ensure Box::leak doesn't leak memory, the pointer doesn't move
            let v = unsafe { Box::from_raw(t) };
            if !t.is_null() {
                Some(v)
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
