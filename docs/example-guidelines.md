# Example guidelines

Examples should look like ordinary Axum applications. Name application state
`AppState`, install it with `Router::with_state`, and extract it with
`State<AppState>`. Keep routers compact and move nontrivial work into named
handlers. Inline handlers are reserved for a complete response that is one
obvious expression and needs no application state.

Beginner pages should have at most three top-level props. Prefer small typed
domain values such as `todos`, `stats`, `errors`, `user`, and `projects` over
large JSON objects. Teach deferred loading, validation, and other policies one
at a time; advanced protocol fixtures must identify themselves as such.

Comments should explain Inertia-specific behavior, such as why a deferred prop
is absent initially or why validation redirects. Do not comment ordinary Rust
operations such as constructing a router, cloning state, or returning a value.

The recommended documentation path uses `InertiaPage`, `InertiaForm`,
`Validated<T>`, `Prop<T>`, `defer`, `RouterInertiaExt`, and
`inertia-axum-test::TestApp`. Low-level protocol types and compatibility APIs
belong in reference or migration material rather than the getting-started flow.
