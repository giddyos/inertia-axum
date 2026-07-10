# Axum Adapter Audit

Audit date: 2026-05-15

This audit checks the Axum integration against the supported dependency floors:

- `axum` 0.8.9
- `tower` 0.5.3
- `fluent-uri` 0.4.1

The audit is intentionally read-only. It records whether the adapter code is
aligned with the current framework APIs and identifies follow-up work that
should be reviewed separately.

## Summary

The Axum adapter is aligned with the supported dependency floors. Its custom
`VersionLayer`/`VersionService` and `InertiaRequest` extractor are appropriate
for the current public API. The only notable design tradeoff is the shared-prop
extension snapshot used by `InertiaRequest::extension`; it is documented and
covered by shared-prop tests, so it should not change without a dedicated API
review.

## Findings

| Area | Finding | Follow-up |
| --- | --- | --- |
| Extractor | `InertiaRequest` implements `FromRequestParts<S>` with `S: Send + Sync`, matching Axum 0.8 extractor expectations. | None. |
| Version middleware | The custom Tower `Layer`/`Service` inserts a version extension and short-circuits stale Inertia GET requests before handlers run. | Keep as-is unless a future API redesign replaces it with a higher-level helper. |
| Shared props | `SharedProps` via `Extension` is idiomatic Axum and keeps shared props opt-in. | None. |
| Extension access | `InertiaRequest` snapshots extensions only when `SharedProps` is present, then removes the registry before provider access. | If this becomes a performance concern, evaluate a typed state extractor or first-class layer separately. |
| Handler ergonomics | `request.render(...)`, `request.location(...)`, and `request.redirect(...)` are explicit and avoid manual protocol response construction. | None. |

## Follow-up Candidates

These are intentionally outside this audit:

- Consider a first-class Axum shared-props layer only if applications find
  `Extension(SharedProps)` confusing or extension snapshot costs show up in
  profiling.
- Revisit the hand-written Axum `VersionService` only if Axum or Tower
  introduces a simpler primitive that preserves the current public API.

## Verification Notes

The test suite exercises Axum HTML and JSON page responses, version conflicts,
redirects, shared props, lazy props, extension access, partial reloads, scroll
metadata, history flags, and not-found passthrough. Framework-neutral tests
cover request parsing and page-object serialization.
