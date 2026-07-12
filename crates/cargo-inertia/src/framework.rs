//! Supported frontend frameworks.

/// A frontend framework that can be scaffolded by `cargo inertia`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Framework {
    /// Svelte.
    Svelte,
    /// React.
    React,
    /// Vue.
    Vue,
}
