use super::{Frame, Protocol, WireObject};

mod file;
pub use file::Loop;

/// Trait that defines fns to transport wire data,
///
/// Follows the visitor pattern,
///
pub trait Transport<T>
where
    T: WireObject,
{
    /// Returns the next wire object from the transport,
    ///
    /// Return None to indicate that the transport is closed,
    ///
    fn next(&mut self) -> Option<T>;

    /// Transport frame,
    ///
    fn transport(&mut self, frame: &Frame);

    /// Transport a control data frame,
    ///
    fn transport_control_data(&mut self, frame: &Frame);

    /// Transports a control index frame,
    ///
    fn transport_control_index(&mut self, frame: &Frame);

    /// Transport a control read frame,
    ///
    fn transport_control_read(&mut self, frame: &Frame);

    /// Called when frame transport starts,
    ///
    fn start_frame(&mut self);

    /// Callend when transport ends,
    ///
    fn end_frame(&mut self);

    /// Called when starting to transport an object,
    ///
    fn start_object(&mut self);

    /// Called when finished transporting an object,
    ///
    fn end_object(&mut self);

    /// Called when starting to transport control data,
    ///
    /// This data is required for decoding the subsequent objects,
    ///
    fn start_control_data(&mut self);

    /// Called when starting to transport the rest of the control data,
    ///
    fn start_control(&mut self);

    /// Calledn when transporting the control device has completed,
    ///
    fn end_control(&mut self);

    /// Sends wire objects from a protocol,
    ///
    fn send(&mut self, protocol: &Protocol) {
        if let Some(control_device) = protocol.control_device::<T>() {
            self.start_control_data();
            for data in control_device.data_frames() {
                self.transport_control_data(&data);
            }

            self.start_control();
            for read in control_device.read_frames() {
                self.transport_control_read(&read);
            }

            for index in control_device.index_frames() {
                self.transport_control_index(&index);
            }
            self.end_control();

            for object in protocol.iter_object_frames::<T>() {
                self.start_object();
                self.transport_frames(object);
                self.end_object();
            }
        }
    }

    /// Send frames,
    ///
    fn transport_frames(&mut self, frames: &[Frame]) {
        for frame in frames {   
            self.start_frame();
            self.transport(frame);
            self.end_frame();
        }
    }
}
