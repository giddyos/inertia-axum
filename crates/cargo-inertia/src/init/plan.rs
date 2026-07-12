//! In-memory scaffold plan.

use std::path::PathBuf;

/// One rendered output file.
pub struct RenderedFile {
    /// Safe path relative to destination.
    pub relative_path: PathBuf,
    /// Exact UTF-8 source bytes.
    pub contents: Vec<u8>,
}
/// The complete scaffold, rendered before any destination is created.
pub struct ScaffoldPlan {
    /// Final frontend directory.
    pub destination: PathBuf,
    /// All output files.
    pub files: Vec<RenderedFile>,
}
