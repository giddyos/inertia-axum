//! Package-manager selection shared by CLI commands.

/// A JavaScript package manager supported by generated projects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageManager {
    /// npm.
    Npm,
    /// pnpm.
    Pnpm,
    /// Bun.
    Bun,
}
