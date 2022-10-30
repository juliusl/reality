use std::{io::Cursor, collections::{VecDeque, HashMap}};

use tracing::{event, Level};

use crate::{Block, wire::{ControlDevice, Interner, Frame, Protocol, WireObject}};

use super::Transport;

/// Struct for a loop transport, (Testing)
/// 
#[derive(Default)]
pub struct Loop {
    protocol: Option<Protocol>,
    control_device: Option<ControlDevice>,
    interner: Option<Interner>,
    blob: Cursor<Vec<u8>>,
    transporting: Vec<Frame>,
    current: Option<Frame>,
    blocks: VecDeque<Block>,
    /// Map of frames that are extents,
    /// 
    /// Since blob data cannot be inlined into a frame, they are encoded as extents. This is a 
    /// map of frame to extent. The frame can be used as an id for each block of blob data.
    /// 
    blobs: HashMap<Frame, (u64, u64)>
}

impl Transport<Block> for Loop {
    fn next(&mut self) -> Option<Block> {
        self.blocks.pop_front()
    }

    fn transport_control_data(&mut self, frame: &crate::wire::Frame) {
        if let Some(control_device) = self.control_device.as_mut() { 
            control_device.data.push(frame.clone());
        }
    }

    fn transport_control_index(&mut self, frame: &crate::wire::Frame) {
        if let Some(control_device) = self.control_device.as_mut() {
            control_device.index.push(frame.clone());
        }
    }

    fn transport_control_read(&mut self, frame: &crate::wire::Frame) {
        if let Some(control_device) = self.control_device.as_mut() {
            control_device.read.push(frame.clone());
        }
    }

    fn transport(&mut self, frame: &crate::wire::Frame) {
        self.current = Some(frame.clone());
    }

    fn start_frame(&mut self) {
        event!(Level::DEBUG, "Transporting frame starting");
        self.current = Some(Frame::default());
    }

    fn end_frame(&mut self) {
        event!(Level::DEBUG, "Transporting frame ending");
        if let Some(frame) = self.current.take(){
            self.transporting.push(frame);
        }
    }

    fn start_object(&mut self) {
        event!(Level::DEBUG, "Transporting object starting");
        self.transporting.clear();
    }

    fn end_object(&mut self) {
        event!(Level::DEBUG, "Transporting object ending");
        if let (Some(protocol), Some(interner)) = (self.protocol.as_ref(), self.interner.as_ref()) {

            let block = Block::decode(&protocol, &interner, &self.blob, &self.transporting);

            self.blocks.push_back(block);
        }
    }

    fn start_control_data(&mut self) {
        event!(Level::DEBUG, "Transporting control data frames starting");
        self.control_device = Some(ControlDevice::default());
    }

    fn start_control(&mut self) {
        event!(Level::DEBUG, "Transporting control frames starting");
        self.protocol = Some(Protocol::empty());
    }

    fn end_control(&mut self) {
        event!(Level::DEBUG, "Transporting control device ending");

        if let Some(control_device) = self.control_device.take() {
            self.interner = Some(control_device.into());
        }
    }
}

#[test]
#[tracing_test::traced_test]
fn test_loop() {
    use crate::Parser;

    let protocol = Protocol::new(Parser::new().parse(r#"
    ```
    + engine .empty
    : start .symbol setup
    : start .symbol start
    : exit  .empty
    ```

    ``` setup
    + runtime .empty
    : println .symbol hello world
    ```

    ``` start
    + runtime .empty
    : println .text goodbye world
    ```
    "#));

    // let mut loop_device = Loop::default();

    // loop_device.send(&protocol);

    // while let Some(block) = loop_device.next() {
    //     eprintln!("{:#?}", block);
    // }
}