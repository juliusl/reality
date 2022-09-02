use bytemuck::cast;

use crate::wire::Frame;

/// Struct for organizing control data for transport by frame,
///
/// Control data is required to be transferred across the wire before
/// any other frame can be decoded by the wire protocol.
/// To normalize all wire data to a frame unit, control data must operate on  
/// a more compact data transfer protocol. Since the first type of control data
/// are identifiers and symbols, the lowest common denominator are short length strings.
///
/// The control data transfer is organized by a control device, that can be consumed
/// into a vector of frames. The control device organizes data by classes. As the
/// data class increases, the read length increases.
///
/// When the device converts control data into frames, multiple pieces of control
/// data are packed onto two types of frames, data and reads.
///
/// Data frames build a data buffer on the other side of the wire, and read frames
/// instruct the other side of the wire how to advance the cursor.
///
/// A class of reads completes by appending a `0` to the frame data. (Though this is
/// for the purpose of tracking parity and progress)
///
/// Frames must be transferred in the order they were created in the origin.
///
#[derive(Clone, Default)]
pub struct ControlBuffer {
    class_1: Vec<String>,
    class_2: Vec<String>,
    class_3: Vec<String>,
    class_4: Vec<String>,
}

impl ControlBuffer {
    /// Adds a string to the control device
    ///
    pub fn add_string(&'_ mut self, string: impl AsRef<str>) {
        match string.as_ref().len() {
            ref len if *len <= u8::MAX as usize => {
                self.class_1.push(string.as_ref().to_string());
            }
            ref len if *len > u8::MAX as usize && *len <= u16::MAX as usize => {
                self.class_2.push(string.as_ref().to_string());
            }
            ref len if *len > u16::MAX as usize && *len <= u32::MAX as usize => {
                self.class_3.push(string.as_ref().to_string());
            }
            ref len if *len > u32::MAX as usize && *len <= u64::MAX as usize => {
                self.class_4.push(string.as_ref().to_string());
            }
            _ => {}
        }
    }
}

impl Into<Vec<Frame>> for ControlBuffer {
    fn into(self) -> Vec<Frame> {
        let mut data_frames = vec![];
        let mut control_frames = vec![];

        let mut data = vec![];

        let mut class1_reads = vec![];
        for s in self.class_1.iter() {
            let s = s.as_bytes();
            class1_reads.push(s.len() as u8);
            data.push(s);
        }

        let mut class2_reads = vec![];
        for s in self.class_2.iter() {
            class2_reads.push(s.len() as u16);
            data.push(s.as_bytes());
        }

        let mut class3_reads = vec![];
        for s in self.class_3.iter() {
            class3_reads.push(s.len() as u32);
            data.push(s.as_bytes());
        }

        let mut class4_reads = vec![];
        for s in self.class_4.iter() {
            class4_reads.push(s.len() as u64);
            data.push(s.as_bytes());
        }

        // First, construct data frames
        //
        let data = data.concat();
        for c in data.chunks(63) {
            let mut slice = [0; 63];
            slice[..c.len()].copy_from_slice(c);
            let data_frame = Frame::instruction(0x00, &slice);
            data_frames.push(data_frame);
        }

        for r in class1_reads.chunks(63) {
            let mut buf = [0; 63];
            buf[..r.len()].copy_from_slice(r);

            let frame = class_1(buf);
            control_frames.push(frame);
        }

        for r in class2_reads.chunks(31) {
            let mut buf = [0; 31];
            buf[..r.len()].copy_from_slice(r);

            let frame = class_2(buf);
            control_frames.push(frame);
        }

        for r in class3_reads.chunks(15) {
            let mut buf = [0; 15];
            buf[..r.len()].copy_from_slice(r);

            let frame = class_3(buf);
            control_frames.push(frame);
        }

        for r in class4_reads.chunks(7) {
            let mut buf = [0; 7];
            buf[..r.len()].copy_from_slice(r);

            let frame = class_4(buf);
            control_frames.push(frame);
        }

        data_frames.append(&mut control_frames);
        data_frames
    }
}

fn class_1(reads: [u8; 63]) -> Frame {
    Frame::instruction(0x01, &reads)
}

fn class_2(reads: [u16; 31]) -> Frame {
    let mut data = [0; 32];
    data[1..].copy_from_slice(&reads);

    let data = cast::<[u16; 32], [u8; 64]>(data);
    let mut reads = [0; 63];
    reads.copy_from_slice(&data[1..]);

    Frame::instruction(0x02, &reads)
}

fn class_3(reads: [u32; 15]) -> Frame {
    let mut data = [0; 16];
    data[1..].copy_from_slice(&reads);

    let data = cast::<[u32; 16], [u8; 64]>(data);
    let mut reads = [0; 63];
    reads.copy_from_slice(&data[1..]);

    Frame::instruction(0x03, &reads)
}

fn class_4(reads: [u64; 7]) -> Frame {
    let mut data = [0; 8];
    data[1..].copy_from_slice(&reads);

    let data = cast::<[u64; 8], [u8; 64]>(data);
    let mut reads = [0; 63];
    reads.copy_from_slice(&data[1..]);

    Frame::instruction(0x04, &reads)
}

#[test]
#[tracing_test::traced_test]
fn test_control_buffer() {
    use tracing::{event, Level};
    let mut control_device = ControlBuffer::default();

    control_device.add_string("call");
    control_device.add_string("println");
    control_device.add_string("process");
    control_device.add_string("remote");
    control_device.add_string("test");

    let frames: Vec<Frame> = control_device.into();

    let data_frame = frames.get(0).expect("data frame");
    event!(
        Level::TRACE, 
        "{:x?}", 
        data_frame
    );

    let control_frame = frames.get(1).expect("control frame");
    event!(
        Level::TRACE, 
        "{:x?}", 
        control_frame
    );
}
