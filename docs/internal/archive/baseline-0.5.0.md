# `0.5.0` redesign baseline

This is the phase-0 regression record for the `1.0.0-alpha.1` DX redesign. It
captures the supplied repository state before application-facing API work. The
baseline commit is `46c669c36c1e290a66a3f3c77d0beedfa40c0845`, which descends from
the verified source commit `c853e2ea02d5bc762e219145121a592fd0d40fbd`.

## Verification record

Recorded on July 10, 2026, on an Apple M1 Max (`aarch64-apple-darwin`, macOS
26.5) with Cargo and rustc 1.96.0. Rust 1.88 remains the package MSRV and the
CI MSRV gate.

| Gate | Result |
| --- | --- |
| `cargo test --locked --workspace --all-targets` | passed: 100 tests in 4 suites |
| `cargo bench --workspace --no-run` | passed: all four Criterion targets and workspace targets built |
| Protocol snapshots | 33 committed snapshots; combined SHA-256 `22e336c28d351ad4586204f76dacc0ec08d70109bb28772155f9008731134353` |
| Criterion baseline | saved locally as `phase0-0.5.0` under the ignored `target/criterion` tree |

The same-machine point estimates recorded by Criterion were:

| Scenario | Estimate |
| --- | ---: |
| `request_context/empty` | 19.462 ns |
| `request_context/all_headers` | 568.84 ns |
| `request_context/partial_data/1` | 249.60 ns |
| `request_context/partial_data/8` | 244.55 ns |
| `request_context/partial_data/32` | 262.41 ns |
| `request_context/partial_data/128` | 283.91 ns |
| `page_render/ordinary/1024` | 1.6182 us |
| `page_render/ordinary/65536` | 89.428 us |
| `page_render/ordinary/1048576` | 1.4957 ms |
| `page_render/script_sensitive_64k` | 169.08 us |
| `shared_props/clone` | 10.261 ns |
| `version_layer/clone_static` | 9.9623 ns |

The workspace-wide `cargo bench -- --save-baseline ...` form cannot be used
because Cargo also forwards the Criterion-only argument to the example binary
test harnesses. Each of the four named bench targets was therefore measured
explicitly. This does not affect the required workspace benchmark-build gate.

## Package and repository facts

The baseline is one published `inertia-axum` crate at version 0.5.0, crate path
`inertia_axum`, edition 2021, Rust 1.88 MSRV, and MIT license. Its dependency
floors include Axum 0.8.9 and Tower 0.5.3. The resolver is 2. The two workspace
examples are `examples/axum-minimal` and `examples/axum-svelte`. There is no
proc-macro crate, dedicated test crate, or built-in Vite runtime.

The modular implementation under `src/{axum,html,page,props,request,shared}` is
a migration asset. The `tests/protocol_v3` in-process suite and all 33 protocol
snapshots remain authoritative. The four existing Criterion groups are
`request_context`, `page_render`, `shared_props`, and `version_layer`.

The current public application surface comprises `Inertia<T>`,
`InertiaPageBuilder`, `InertiaProps`, `ScopedInertiaProps<'a>`,
`IntoPageProps`, `InertiaRequest`, `SharedProps`, `SharedRequest`,
`InertiaVersion`, `VersionLayer`, `VersionService`, `RequestContext`, `Page<T>`,
`PageMetadata`, `OnceProp`, `ScrollProps`, `Redirect`, `Location`,
`HtmlResponseContext`, `InertiaError`, and the public protocol header constants.

## Carry-forward protocol, security, and performance invariants

- Parse the complete implemented Inertia v3 request-header surface without
  eagerly materializing header lists into `Vec<String>` values.
- Select props before invoking resolvers. `except` wins over `only`; component
  mismatch disables partial selection; writes ignore ordinary partial filtering
  but retain required once exclusions.
- Preserve stable page-object field order, cheaply cloned string versions,
  omitted empty metadata, and an object-valued `errors` prop on object props.
- Preserve history, merge, scroll, deferred, rescued, shared-root, once, reset,
  and infinite-scroll metadata semantics, pruning metadata for absent props.
- Keep the one-pass script-safe initial-page serializer and its protection for
  closing script tags, ampersands, and JavaScript line separators.
- Preserve route-owned, route-local shared, then global shared collision
  precedence; dotted-key expansion; and deduplicated shared roots.
- Short-circuit stale Inertia GET versions before the handler, while avoiding
  dynamic-version resolution on unrelated paths.
- Preserve method-aware redirects, Inertia location conflicts, fragment redirect
  headers, typed invalid-URI errors, accepted relative references, and
  deduplicated `Vary: X-Inertia`.
- Do not clone the full request extension map, restore full-extension snapshots,
  add avoidable response JSON round trips, or duplicate finalization paths.

## Binding target and migration policy

The target is a virtual resolver-3 workspace at `1.0.0-alpha.1`, potentially
edition 2024, while retaining Rust 1.88 unless a concrete dependency requires a
documented change. It adds peer runtime, macro, testing, and optional CLI
packages, but no separately published protocol/core crate. Package metadata
remains MIT and `https://github.com/giddyos/inertia-axum`.

Existing protocol models and header constants stay source-compatible during
migration. New direct responses must share the finalizer with compatibility
extractor methods. `ScopedInertiaProps<'a>` remains available for immediate
borrowed rendering; direct responses require owned or static data. Compatibility
support remains through the alpha period and migration documentation must give
mechanical before/after examples. `Page<T>` remains the wire object while
untyped direct responses use `DynamicPage`. Typed pages use a distinct
`InertiaPage` boundary rather than overlapping the blanket `IntoPageProps`
implementation.

The first design intentionally excludes controller/route macros,
`#[inertia::main]`, process-global sharing, per-route HTML callbacks, multiple
string-keyed shared providers, ORM integration, frontend-framework-specific Rust
features, proc-macro TypeScript file generation, a mandatory CLI, a mandatory
session dependency, a separate protocol crate, SSR, and precognition.

Future phases must retain the existing CI gates and expand them to cover the new
packages, package ordering, all features, no-default-features, docs, Rust 1.88,
frontend fixtures, and benchmark builds. They must also add the specified
pending-finalization, async-selection/concurrency, Vite, and transient-state
benchmarks without removing the four baseline groups.

No phase 1-9 application-facing API was introduced while producing this record.
