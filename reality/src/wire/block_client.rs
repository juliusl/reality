use std::ops::Range;
use tokio::{io::AsyncRead, task::JoinHandle};

mod block_entry;
pub use block_entry::BlockEntry;

/// Type alias over a task that returns a list of block entries,
/// 
pub type ListBlocks<T> = JoinHandle<std::io::Result<Vec<T>>>;

/// Trait to enable different block client implementations,
///
pub trait BlockClient: Clone + Send + Sync + 'static {
    /// Type of stream being read from, ex. file, duplex, etc,
    ///
    type Stream: AsyncRead + Unpin + Send + Sync + 'static;

    /// Concrete type implementing the block entry trait,
    /// 
    type Entry: BlockEntry;

    /// Returns a stream to read a range of bytes,
    /// 
    fn stream_range(&self, range: Range<usize>) -> Self::Stream;

    /// Returns a stream to transport a range of bytes,
    /// 
    fn transport_range(&self, range: Range<usize>) -> Self::Stream;

    /// Returns a join handle whose result if successful is a vector of block entries,
    /// 
    fn list_blocks(&self) -> ListBlocks<Self::Entry>;
}


mod test {
    use crate::wire::Frame;
    use super::{BlockClient, BlockEntry};

    #[derive(Clone)]
    struct TestBlockClient {
        data: &'static [u8],
        blocks: Vec<TestBlockEntry>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestBlockEntry(Frame, usize);

    impl BlockEntry for TestBlockEntry {
        fn frame(&self) -> Frame {
            self.0.clone()
        }

        fn size(&self) -> usize {
            self.1
        }
    }

    impl BlockClient for TestBlockClient {
        type Stream = &'static [u8];

        type Entry = TestBlockEntry;

        fn stream_range(&self, range: std::ops::Range<usize>) -> Self::Stream {
            self.transport_range(range)
        }

        fn list_blocks(&self) -> super::ListBlocks<Self::Entry> {
            tokio::task::spawn(std::future::ready(Ok(self.blocks.clone())))
        }

        fn transport_range(&self, range: std::ops::Range<usize>) -> Self::Stream {
            &self.data[range.start..range.end]
        }
    }

    #[tokio::test]
    async fn test_block_client() {
        use std::io::Read;

        let test_entry = TestBlockEntry(Frame::extension("test", "entry_1"), 10);
        let client = TestBlockClient {
            data: b"Hello World", 
            blocks: vec![
                test_entry.clone()
            ]
        };

        let mut test = String::new();
        client.stream_range(0..11).read_to_string(&mut test).expect("should be able to read");
        assert_eq!(test, "Hello World");

        let list = client.list_blocks().await.expect("should return task").expect("should return block list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], test_entry);
    }
}