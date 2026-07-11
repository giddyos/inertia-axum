# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

## 1.0.0-alpha.1 - 2026-07-10

### Added

- Direct `DynamicPage` and derived `InertiaPage` handler responses with typed
  prop keys.
- Concurrent lazy, optional, deferred, once, rescue, merge, and infinite-scroll
  prop policies.
- Convention-based Vite setup, typed shared data, transient flash, forms, and
  redirect-back validation.
- The separate `inertia-axum-test` package and production-style Todo, incident,
  and observatory fixtures.
- Optional `cargo inertia init`, `dev`, and `check` commands.

### Changed

- Moved the runtime into a peer crate under a resolver-3 virtual workspace.
- Preserved the 0.5 compatibility APIs while routing new direct responses
  through the common finalizer.
- Set the workspace MSRV to Rust 1.88 and the package edition to 2024.

See [the migration guide](docs/content/docs/migrations/from-0-5.mdx) for mechanical upgrades.

## 0.5.0 - 2026-07-10

### Changed

- Moved public crate entry points into a modular source layout while preserving
  existing public paths.
- Unified eager and lazy prop inclusion decisions and removed temporary
  per-prop selection sets.
- Removed cloned materialized prop keys during partial-reload filtering.
- Deduplicated metadata at insertion time and consume metadata while building
  the response instead of cloning it.
- Added capacity-aware lazy prop construction.
- Replaced the version middleware's boxed future with a concrete future.
- Deferred dynamic version providers until a route extracts `InertiaRequest`.
- Removed `InertiaRequest::extension`; shared providers use a narrow request
  view and extraction no longer snapshots all Axum extensions.
- Added route-local shared values through `shared_value` and
  `serialize_shared`.
- Serialize HTML `data-page` JSON in one script-safe pass.

### Added

- Workspace-wide benchmark targets and a recorded request-context baseline.
- Workspace-aware CI, benchmark compilation, and frontend example build.

### Compatibility

- Made the Axum integration unconditional and removed framework-selection
  feature flags.

### Removed

- Removed the legacy alternate web-framework adapter, its public API,
  dependencies, examples, documentation, and CI coverage.

## 0.4.0 - 2026-05-15

### Added

- Public Inertia protocol header constants and `RequestContext` parsing.
- Inertia v3 page-object metadata for history flags, merge props, deferred
  props, once props, shared props, and scroll props.
- Partial reload filtering for matching Inertia components.
- Asset version handling for successful page responses and stale visits.
- `Inertia::location` for external Inertia redirects.
- `Inertia::redirect` for method-aware application redirects.
- Initial Axum integration with `InertiaRequest`, `VersionLayer`, page response
  rendering, external locations, and method-aware redirects.
- Minimal Axum example.
- `InertiaProps` and `ScopedInertiaProps` for synchronous lazy, optional,
  deferred, and once prop resolvers.
- Axum `SharedProps` extension support for common page props.
- README protocol support matrix.

### Changed

- Preserved query strings in generated page object URLs.
- Added `Vary: X-Inertia` to responses that vary between HTML and JSON.
- Modernized GitHub Actions CI and declared an MSRV of Rust 1.88.
- Expanded crate and public API rustdoc.

### Fixed

- Escaped serialized page JSON for safe embedding in HTML script contexts.
- Kept shared dotted props from merging into route-owned prop roots.
- Preserved route-owned prop roots across partial filtering before shared-prop
  merging.
- Kept internal route-prop tracking out of `Page` equality.
