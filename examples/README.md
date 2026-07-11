# Examples

The browser examples include frontend assets and run a local server. The other
examples are in-process Axum applications whose behavior is demonstrated by
focused tests.

| Example | Purpose | Command |
| --- | --- | --- |
| [`axum-minimal`](axum-minimal) | Smallest Axum router, `AppState`, initial HTML, and Inertia JSON | `cargo run -p axum-minimal` |
| [`axum-svelte`](axum-svelte) | Axum, Vite, Svelte 5, automatic deferred loading, and validation UI | `cargo run -p axum-svelte` |
| [`todo`](todo) | Canonical typed page, validation, deferred prop, and `TestApp` tests | `cargo test -p inertia-axum-example-todo` |
| [`incident-board`](incident-board) | Advanced rescue, merge, scroll, flash, and external locations | `cargo test -p inertia-axum-example-incident-board` |
| [`observatory`](observatory) | Protocol regression fixture for once props, reset, deep merge, and redaction | `cargo test -p inertia-axum-example-observatory` |
