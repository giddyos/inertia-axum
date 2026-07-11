# Examples

The browser examples include frontend assets and run a local server. The other
examples are in-process Axum applications whose behavior is demonstrated by
focused tests.

| Example | Purpose | Command |
| --- | --- | --- |
| [`axum-minimal`](axum-minimal) | Smallest Axum router, `AppState`, initial HTML, and Inertia JSON | `cargo run -p axum-minimal` |
| [`axum-askama`](axum-askama) | Typed Askama root document with a minimal Vue page | Build the frontend, then `cargo run -p axum-askama` |
| [`axum-svelte`](axum-svelte) | Svelte 5, deferred data, validation, managed SSR, and route policies | `cargo run -p axum-svelte` |
| [`axum-react`](axum-react) | React 19, deferred data, validation, managed SSR, and route policies | `cargo run -p axum-react` |
| [`axum-vue`](axum-vue) | Vue 3, deferred data, validation, managed SSR, and route policies | `cargo run -p axum-vue` |
| [`todo`](todo) | Canonical typed page, validation, deferred prop, and `TestApp` tests | `cargo test -p inertia-axum-example-todo` |
| [`incident-board`](incident-board) | Advanced rescue, merge, scroll, flash, and external locations | `cargo test -p inertia-axum-example-incident-board` |
| [`observatory`](observatory) | Protocol regression fixture for once props, reset, deep merge, and redaction | `cargo test -p inertia-axum-example-observatory` |

See the [examples reference](../docs/content/docs/reference/examples.mdx) for feature
requirements, build artifacts, intended audiences, and production limitations.
