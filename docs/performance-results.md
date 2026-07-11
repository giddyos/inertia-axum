# Performance Results

Base commit: `110dcf0d345b0e78676d40d8a68661c1e271ecd3`

Final commit is recorded when this release branch is finalized. Measurements
were collected on an Apple M1 Max running macOS 26.5 with rustc 1.96.0.

The baseline point estimates and 95% confidence intervals are in
[`performance-baseline.md`](performance-baseline.md). This release avoids
claiming a percentage improvement until the same Criterion scenarios are run
against both base and final commits.

Implemented allocation reductions include retain-based materialized prop
filtering, metadata consumption rather than response-path cloning, capacity
aware lazy props, and a concrete version middleware future. The unavoidable
remaining allocations include owned protocol header values, serialized JSON,
and page-object strings.

Dynamic version providers are now deferred for non-Inertia middleware paths,
so routes that never extract `InertiaRequest` do not invoke them.

## Phase 1 response-finalization checkpoint

The four `0.5.0` groups were rerun on the same machine against the saved
`phase0-0.5.0` Criterion baseline after response finalization was added.
`request_context`, `shared_props`, and the large and script-sensitive page
render scenarios showed no consistent regression. Criterion intermittently
flagged small nanosecond-scale changes in `partial_data/8` and
`version_layer/clone_static`, although neither implementation is touched by
phase 1. The 64 KiB serializer scenario varied from 89.4 us at baseline to
91-92 us in the checkpoint, while the 1 MiB scenario improved from 1.50 ms to
1.44 ms. A repeat reduced the 1 KiB and script-sensitive changes to Criterion's
noise threshold. These mixed directions on byte-for-byte unchanged benchmark
code are recorded as host/load variance rather than an attributed product
regression; the raw baselines remain under `target/criterion` for subsequent
same-machine comparisons.

Phase 1 adds `pending_page_finalize`. Its first saved
`phase1-response-finalization` measurements were 3.54 us for an Inertia JSON
response and 3.40 us for an initial HTML response. Both exercise the router,
concrete middleware future, request-local pending handle, existing page draft,
serialization, and final response construction.

## Phase 2 Vite checkpoint

`vite_initial_html` builds the Vite provider once before measurement, then
benchmarks the complete initial-page router path. The saved `phase2-vite`
baseline measured 2.98 us. Manifest I/O, parsing, graph traversal, and stable
version hashing are intentionally absent from the request-time measurement
because production setup performs them once during `InertiaApp::build()`.
