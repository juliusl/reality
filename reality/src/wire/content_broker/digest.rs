use std::io::Write;

use sha2::Digest;
use tracing::event;
use tracing::Level;

use crate::wire::BlobSource;
use crate::wire::ContentBroker;
use crate::wire::MemoryBlobSource;

/// SHA256 digester formats the blob device address to be
/// the sha256 digest of the blob device's content
/// 
pub struct Sha256Digester();

impl ContentBroker for Sha256Digester {
    fn format(&mut self, ref source: impl crate::wire::BlobSource) -> MemoryBlobSource {
        let mut output = MemoryBlobSource::default();

        for (address, blob_device) in source.hash_map() {
            event!(Level::TRACE, "Formatting {address}");

            let mut cursor = blob_device.consume();
            cursor.set_position(0);

            let mut digester = sha2::Sha256::new();

            let content = &cursor.clone().into_inner();
            match digester.write_all(content) {
                Ok(_) => {
                    event!(Level::TRACE, "Formatted {address}");
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not create digest for {address}, {err}");
                }
            }

            // TODO: I'm sure there is a better way
            let digest = format!("sha256:{:x?}", digester.finalize())
                .replace('[', "")
                .trim_end_matches(']')
                .split(", ")
                .collect::<Vec<_>>()
                .join("");

            let new_device = output.new(&digest);
            match new_device.as_mut().write_all(content) {
                Ok(_) => {
                    event!(
                        Level::DEBUG,
                        "Completed formatting\n\t{address} -> {digest}"
                    );
                }
                Err(err) => {
                    event!(
                        Level::ERROR,
                        "Could not complete formatting\n\t{address} -> {digest}\n\t{err}"
                    );
                }
            }
        }

        output
    }
}

/// Tests formatting a blob source w/ digester 
/// 
#[test]
#[tracing_test::traced_test]
fn test_sha256_digester() {
    // Create a temp blob source
    let mut test = MemoryBlobSource::default();

    // Add a blob to the temp source and write some data
    let test_value = test.new("test_value");
    test_value
        .as_mut()
        .write_all(b"hello world")
        .expect("can write");

    // Reformat blobs in the temp source
    let digested_source = Sha256Digester().format(test);
    
    // Verify the resulting address that is created 
    for (a, _) in digested_source.hash_map() {
        assert_eq!("sha256:b94d27b9934d3e8a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9", a);
    }
}
