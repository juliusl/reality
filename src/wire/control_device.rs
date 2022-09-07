use super::{Frame, Interner};
use bytemuck::cast;
use std::{
    collections::{BTreeSet, HashMap},
    io::{Cursor, Read, Write},
};
use tracing::{event, Level};

mod control_buffer;
pub use control_buffer::ControlBuffer;

/// Control device arranges control frames,
///
/// When blocks are encoded, values are normalized into a 64 byte frame. Most values
/// can be inlined into the frame except for values that contains identifiers and binary data.
/// Identifiers are stored in an interner, and since identifiers are required to decode blocks,
/// the interner must be exist first. The control device allows the decoding process to
/// bootstrap itself, so that it minimizes the amount of data that needs to be transferred before
/// the rest of the blocks can be decoded.
///
#[derive(Default, Debug)]
pub struct ControlDevice {
    data: Vec<Frame>,
    read: Vec<Frame>,
    index: Vec<Frame>,
}

impl ControlDevice {
    /// Returns a new control device from an interner,
    ///
    pub fn new(interner: Interner) -> Self {
        let mut control_device = ControlDevice::default();
        let mut control_buffer = ControlBuffer::default();
        for (_, ident) in interner.strings() {
            control_buffer.add_string(ident);
        }

        for (_, complex) in interner.complexes() {
            control_buffer.add_complex(complex);
        }

        let frames: Vec<Frame> = control_buffer.into();

        for frame in frames.iter() {
            if frame.op() == 0x00 {
                control_device.data.push(frame.clone());
            } else if frame.op() > 0x00 && frame.op() < 0x06 {
                control_device.read.push(frame.clone());
            } else {
                assert!(
                    frame.op() >= 0xC1 && frame.op() <= 0xC6,
                    "Index frames have a specific op code range"
                );
                control_device.index.push(frame.clone());
            }
        }

        control_device
    }

    /// Returns the size in bytes of the control device,
    ///
    pub fn size(&self) -> usize {
        self.len() * 64
    }

    /// Returns the length in frames,
    /// 
    pub fn len(&self) -> usize {
        self.data.len() + self.read.len() + self.index.len()
    }

    /// Returns an iterator over data frames,
    ///
    pub fn data_frames(&self) -> impl Iterator<Item = &Frame> {
        self.data.iter()
    }

    /// Returns an iterator over read frames,
    ///
    pub fn read_frames(&self) -> impl Iterator<Item = &Frame> {
        self.read.iter()
    }

    /// Returns an iterator over index frames,
    ///
    pub fn index_frames(&self) -> impl Iterator<Item = &Frame> {
        self.index.iter()
    }

    /// Returns all of the data from data frames,
    ///
    pub fn data(&self) -> Cursor<Vec<u8>> {
        let mut cursor = Cursor::<Vec<u8>>::new(vec![]);

        for data in self.data_frames() {
            match cursor.write(&data.data()) {
                Ok(_) => {}
                Err(err) => event!(Level::ERROR, "could not write to cursor, {err}"),
            }
        }

        cursor.set_position(0);
        cursor
    }
}

/// Consumes the control device and creates an Interner
///
/// TODO: Make this DRY
/// 
impl Into<Interner> for ControlDevice {
    fn into(self) -> Interner {
        let mut interner = Interner::default();
        let mut data = self.data();
        let mut idents = vec![];

        for read in self.read_frames() {
            match read.op() {
                // Class 1 means each read is a u8
                0x01 => {
                    for read in read.data().bytes().filter_map(|b| match b {
                        Ok(b) => Some(b),
                        Err(_) => None,
                    }) {
                        if read != 0x00 {
                            let mut buf = vec![0; read as usize];
                            data.read(&mut buf).expect("can read");
                            let ident = String::from_utf8(buf).expect("is a utf8 string");
                            interner.add_ident(&ident);
                            idents.push(ident);
                        }
                    }
                }
                // Class 2 means each read is a u16
                0x02 => {
                    let reads = cast::<[u8; 64], [u16; 32]>(*read.bytes());
                    let reads = &reads[1..];

                    for read in reads {
                        if *read != 0x00 {
                            let mut buf = vec![0; *read as usize];
                            data.read_exact(&mut buf).expect("can read");
                            let ident = String::from_utf8(buf).expect("is a utf8 string");
                            interner.add_ident(&ident);
                            idents.push(ident);
                        }
                    }
                }
                // Class 3 means each read is a u32
                0x03 => {
                    let reads = cast::<[u8; 64], [u32; 16]>(*read.bytes());
                    let reads = &reads[1..];

                    for read in reads {
                        if *read != 0x00 {
                            let mut buf = vec![0; *read as usize];
                            data.read_exact(&mut buf).expect("can read");
                            let ident = String::from_utf8(buf).expect("is a utf8 string");
                            interner.add_ident(&ident);
                            idents.push(ident);
                        }
                    }
                }
                // Class 4 means each read is a u64
                0x04 => {
                    let reads = cast::<[u8; 64], [u64; 8]>(*read.bytes());
                    let reads = &reads[1..];

                    for read in reads {
                        if *read != 0x00 {
                            let mut buf = vec![0; *read as usize];
                            data.read_exact(&mut buf).expect("can read");
                            let ident = String::from_utf8(buf).expect("is a utf8 string");
                            interner.add_ident(&ident);
                            idents.push(ident);
                        }
                    }
                }
                _ => {
                    event!(Level::WARN, "didnt read {:?}", read);
                }
            }
        }

        let mut intermediate_complex_index = HashMap::<u64, BTreeSet<String>>::default();
        for index in self.index_frames() {
            let data = index.data();
            let mut ident = [0; 8];
            ident.copy_from_slice(&data[..8]);

            let ident_key = cast::<[u8; 8], u64>(ident);
            if let Some(ident) = idents.get(ident_key as usize) {
                match index.op() {
                    0xC1 => {
                        // ident belongs to 1 complex
                        let mut complex_1 = [0; 8];
                        complex_1.copy_from_slice(&data[8..16]);
                        let complex_1 = cast::<[u8; 8], u64>(complex_1);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_1) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_1, set);
                        }
                    }
                    0xC2 => {
                        // ident belongs to 2 complex
                        let mut complex_1 = [0; 8];
                        complex_1.copy_from_slice(&data[8..16]);
                        let complex_1 = cast::<[u8; 8], u64>(complex_1);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_1) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_1, set);
                        }

                        let mut complex_2 = [0; 8];
                        complex_2.copy_from_slice(&data[16..24]);
                        let complex_2 = cast::<[u8; 8], u64>(complex_2);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_2) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_2, set);
                        }
                    }
                    0xC3 => {
                        // ident belongs to 3 complex
                        let mut complex_1 = [0; 8];
                        complex_1.copy_from_slice(&data[8..16]);
                        let complex_1 = cast::<[u8; 8], u64>(complex_1);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_1) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_1, set);
                        }

                        let mut complex_2 = [0; 8];
                        complex_2.copy_from_slice(&data[16..24]);
                        let complex_2 = cast::<[u8; 8], u64>(complex_2);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_2) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_2, set);
                        }

                        let mut complex_3 = [0; 8];
                        complex_3.copy_from_slice(&data[24..32]);
                        let complex_3 = cast::<[u8; 8], u64>(complex_3);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_3) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_3, set);
                        }
                    }
                    0xC4 => {
                        // ident belongs to 4 complex
                        let mut complex_1 = [0; 8];
                        complex_1.copy_from_slice(&data[8..16]);
                        let complex_1 = cast::<[u8; 8], u64>(complex_1);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_1) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_1, set);
                        }

                        let mut complex_2 = [0; 8];
                        complex_2.copy_from_slice(&data[16..24]);
                        let complex_2 = cast::<[u8; 8], u64>(complex_2);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_2) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_2, set);
                        }

                        let mut complex_3 = [0; 8];
                        complex_3.copy_from_slice(&data[24..32]);
                        let complex_3 = cast::<[u8; 8], u64>(complex_3);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_3) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_3, set);
                        }

                        let mut complex_4 = [0; 8];
                        complex_4.copy_from_slice(&data[32..40]);
                        let complex_4 = cast::<[u8; 8], u64>(complex_4);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_4) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_4, set);
                        }
                    }
                    0xC5 => {
                        // ident belongs to 5 complex
                        let mut complex_1 = [0; 8];
                        complex_1.copy_from_slice(&data[8..16]);
                        let complex_1 = cast::<[u8; 8], u64>(complex_1);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_1) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_1, set);
                        }

                        let mut complex_2 = [0; 8];
                        complex_2.copy_from_slice(&data[16..24]);
                        let complex_2 = cast::<[u8; 8], u64>(complex_2);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_2) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_2, set);
                        }

                        let mut complex_3 = [0; 8];
                        complex_3.copy_from_slice(&data[24..32]);
                        let complex_3 = cast::<[u8; 8], u64>(complex_3);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_3) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_3, set);
                        }

                        let mut complex_4 = [0; 8];
                        complex_4.copy_from_slice(&data[32..40]);
                        let complex_4 = cast::<[u8; 8], u64>(complex_4);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_4) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_4, set);
                        }

                        let mut complex_5 = [0; 8];
                        complex_5.copy_from_slice(&data[40..48]);
                        let complex_5 = cast::<[u8; 8], u64>(complex_5);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_5) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_5, set);
                        }
                    }
                    0xC6 => {
                        // ident belongs to 6 complex
                        let mut complex_1 = [0; 8];
                        complex_1.copy_from_slice(&data[8..16]);
                        let complex_1 = cast::<[u8; 8], u64>(complex_1);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_1) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_1, set);
                        }

                        let mut complex_2 = [0; 8];
                        complex_2.copy_from_slice(&data[16..24]);
                        let complex_2 = cast::<[u8; 8], u64>(complex_2);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_2) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_2, set);
                        }

                        let mut complex_3 = [0; 8];
                        complex_3.copy_from_slice(&data[24..32]);
                        let complex_3 = cast::<[u8; 8], u64>(complex_3);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_3) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_3, set);
                        }

                        let mut complex_4 = [0; 8];
                        complex_4.copy_from_slice(&data[32..40]);
                        let complex_4 = cast::<[u8; 8], u64>(complex_4);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_4) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_4, set);
                        }

                        let mut complex_5 = [0; 8];
                        complex_5.copy_from_slice(&data[40..48]);
                        let complex_5 = cast::<[u8; 8], u64>(complex_5);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_5) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_5, set);
                        }

                        let mut complex_6 = [0; 8];
                        complex_6.copy_from_slice(&data[48..52]);
                        let complex_6 = cast::<[u8; 8], u64>(complex_6);

                        if let Some(set) = intermediate_complex_index.get_mut(&complex_6) {
                            set.insert(ident.to_string());
                        } else {
                            let mut set = BTreeSet::default();
                            set.insert(ident.to_string());
                            intermediate_complex_index.insert(complex_6, set);
                        }
                    }
                    _ => {
                        event!(Level::WARN, "didnt convert frame {:?}", index);
                    }
                }
            }
        }

        for (key, set) in intermediate_complex_index.iter() {
            interner.insert_complex(*key, set);
        }

        interner
    }
}

#[test]
#[tracing_test::traced_test]
fn test_device_conversion() {
    let mut interner = Interner::default();
    interner.add_ident("name");
    interner.add_ident("description");
    interner.add_ident("age");
    interner.add_ident("phone");
    interner.add_map(vec!["name", "description", "age"]);

    let control_device = ControlDevice::new(interner.clone());
    event!(Level::TRACE, "{:x?}", control_device);

    let converted_interner: Interner = control_device.into();
    assert_eq!(interner, converted_interner);
}