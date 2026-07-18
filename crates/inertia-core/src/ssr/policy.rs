//! Framework-neutral route-level SSR decision.

/// A request-local override of the application SSR policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SsrOverride {
    /// SSR is explicitly enabled.
    Enabled,
    /// SSR is explicitly disabled.
    Disabled,
}
