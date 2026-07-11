# 1.0.0-alpha.1 release audit

This audit maps the redesign definition of done to durable source or test
evidence. It supplements, rather than replaces, the executable CI gates.

## Phase acceptance evidence

| Phase | Representative evidence |
| --- | --- |
| 00 baseline | `docs/baseline-0.5.0.md`, protocol snapshots, and the four retained baseline benchmarks |
| 01 finalization | `tests/direct_responses.rs`, the request-local `PendingResponseHandle`, and the shared `engine` finalizer |
| 02 Vite | `tests/vite.rs`, `benches/vite_initial_html.rs`, and the mechanically migrated Svelte example |
| 03 props | `tests/unified_props.rs` and the async selection/concurrency/unselected benchmarks |
| 04 macros | macro `trybuild` pass/fail fixtures, renamed-runtime pass coverage, and `tests/typed_pages.rs` |
| 05 shared | `tests/typed_shared.rs` and the retained `shared_props` benchmark |
| 06 transient | `tests/transient_flash.rs` and `benches/transient_cookie_commit.rs` |
| 07 forms | `tests/forms_validation.rs` plus form pass/fail macro fixtures |
| 08 testing | `inertia-axum-test/tests/state.rs` and the Todo, incident-board, and observatory tests |
| 09 CLI/release | CLI unit tests, resolver-3 workspace manifests, package-order CI, migration guide, and README workflow |

## Definition-of-done mapping

1. Direct `DynamicPage` handlers compile without an extractor (`direct_responses`).
2. Those handlers return page values rather than constructing Axum responses.
3. One `InertiaApp` root configures initial HTML rendering.
4. Vite configuration is built once; tests remove the manifest after startup.
5. One typed `Share` provider supplies global shared data.
6. Validation errors and flash use configured transient storage automatically.
7. Execution counters prove unselected async resolvers are not constructed.
8. Selected async resolvers execute concurrently; deferred groups retain independent metadata.
9. Ordinary Axum responses pass through unchanged.
10. Missing layer, Vite, transient, and URI setup failures have focused diagnostics.
11. Page, props, and form derives have compile-pass and compile-fail coverage.
12. Every full Rust example is a workspace member and compiles in all-target CI.
13. `prelude` contains the common application surface rather than protocol internals.
14. `Visit` and compatibility extractors retain advanced protocol access.
15. The README teaches setup, pages, shared data, forms, flash, tests, and optional CLI use.
16. The complete `tests/protocol_v3` snapshot suite remains present.
17. The migration guide documents the compatibility API and new preferred path.
18. Shared preparation borrows request extensions; no cloned extension snapshot exists.
19. Request header lists retain iterator-based, allocation-conscious accessors.
20. Initial page JSON still uses the one-pass script-safe serializer.
21. Typed shared tests prove route, route-local, and global collision precedence.
22. Stale-version tests prove conflict handling runs before handlers.
23. Dynamic-version not-found/ordinary paths prove unrelated responses avoid provider work.
24. All four baseline and six added benchmark targets remain declared.
25. The Svelte example uses `InertiaApp::vite(...)` and passes npm build/test gates.
26. Workspace packages inherit alpha version, edition 2024, Rust 1.88, MIT, and the `giddyos` repository URL.
27. Public examples consistently import `inertia_axum`.
28. Derived pages use the separate `InertiaPage` boundary, avoiding eager-prop coherence overlap.
29. `compatibility_and_direct_paths_emit_byte_equivalent_pages` compares exact response bytes.
30. Runtime source remains modular under `crates/inertia-axum/src`.
31. Vite and test fixtures cover numeric and string version serialization/header comparison.
32. Transient tests prove version conflicts reflash pending state.
33. `Page<T>` remains the wire object and `page!` returns `DynamicPage`.
34. Pending responses use a cloneable, request-local, one-shot handle with no global registry.

## Exclusion audit

The public API and CLI contain no controller/route macro, `#[inertia::main]`,
global share registry, ORM integration, frontend-specific Rust feature, proc
macro file generation, mandatory session/CLI dependency, separate protocol
crate, SSR bridge, or extra `types`, `generate`, `routes`, or migration command.

## Release gates

The release gate is the combined result of formatting, locked all-feature
Clippy, locked all-target tests, doctests, warning-free Rustdoc, no-default
checks, Rust 1.88 and stable checks, package verification in publish order,
benchmark compilation, the existing Svelte npm install/build/test, and real
production builds of all three CLI-generated frontend skeletons.
