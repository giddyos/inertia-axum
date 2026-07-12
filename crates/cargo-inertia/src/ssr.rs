//! Shared server-side rendering configuration types.

/// The rendering mode generated for a frontend project.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SsrMode {
    /// Render only in the browser.
    #[default]
    Csr,
    /// Run the generated Node SSR bundle through `inertia-axum`.
    ManagedNode,
    /// Connect `inertia-axum` to an independently managed SSR endpoint.
    External,
}
