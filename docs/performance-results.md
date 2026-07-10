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
