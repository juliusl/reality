use std::path::PathBuf;

/// Enumeration of runmd Source,
///
/// A source must have a unique relative path name.
///
#[derive(Clone, Debug)]
pub enum Source {
    /// Path to a local file,
    ///
    /// The name of the file will be used to identify this source.
    ///
    Local(PathBuf),
    /// Textbuffer in memory w/ a relative name formatted as a Path,
    ///
    TextBuffer {
        /// Relative path identifier for this buffer,
        ///
        relative: PathBuf,
        /// Source content of this buffer,
        ///
        source: String,
    },
}