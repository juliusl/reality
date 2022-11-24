use bytes::{BytesMut, BufMut};

/// Buffer to allocate buffer space for decoding frames,
/// 
pub struct FrameBuffer {
    /// Buffer of memory space for decoding frames,
    /// 
    buffer: Vec<BytesMut>,
    /// Buffer size,
    /// 
    size: usize,
}

impl FrameBuffer {
    /// Returns a new frame buffer that reserves buffer space for decoding frames,
    /// 
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![BytesMut::with_capacity(64); size],
            size,
        }
    }

    /// Returns the next BytesMut, if buffer is empty, allocates a new BytesMut and resets the buffer,
    /// 
    pub fn next(&mut self) -> BytesMut {
        let mut next = self.buffer.pop().unwrap_or(BytesMut::with_capacity(64));
        next.put_bytes(0, 64);

        if self.buffer.is_empty() {
            self.buffer = vec![BytesMut::with_capacity(64); self.size]; 
        }

        next
    }
}

#[test]
fn test_frame_buffer() {
    let mut buffer = FrameBuffer::new(1);
    assert_eq!(buffer.next().len(), 64);
    assert_eq!(buffer.next().len(), 64);
}