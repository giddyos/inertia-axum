# Migrating to 0.5

Version 0.5 keeps the existing public API paths intact. Most applications can
upgrade without source changes.

`InertiaRequest::extension` has been removed. Extract request extensions in
the handler itself and add request-specific values as route-local shared props:

```rust
async fn dashboard(
    request: inertia_axum::axum::InertiaRequest,
    axum::Extension(user): axum::Extension<CurrentUser>,
) -> Result<axum::response::Response, inertia_axum::axum::InertiaError> {
    request.render(
        inertia_axum::Inertia::response("Dashboard", DashboardProps {})
            .serialize_shared("auth.user", user.summary())?,
        shell,
    )
}
```

Global `SharedProps` providers now receive importable
`inertia_axum::axum::SharedRequest`, which exposes the
Inertia context, method, URI, and asset version. It intentionally does not
expose arbitrary Axum extensions.

The internals no longer rely on cloned materialized prop keys or a boxed future
in version middleware. These are implementation changes only; protocol v3 page
objects and snapshots are intentionally unchanged.

`InertiaProps::with_capacity` is available when an application knows its likely
number of lazy props. It is optional and does not change resolver behavior.

`RequestContext::partial_data`, `partial_except`, `reset`, and
`except_once_props` retain their allocating compatibility return values.
Prefer their corresponding `*_iter` methods in hot paths.
