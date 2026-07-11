# Incident board

This is an advanced fixture, not a recommended starting template.

## What this example teaches

- Advertise and load deferred telemetry.
- Rescue a failed prop without failing the page.
- Emit infinite-scroll and merge metadata.
- Produce external-location responses.
- Carry flash data across redirects.

## Important files

```text
src/lib.rs   State, named handlers, advanced page policies, and focused tests
```

## Routes

| Method | Path | Handler | Purpose |
| --- | --- | --- | --- |
| GET | `/incidents/{id}` | `show` | Render an incident |
| POST | `/incidents` | `store` | Validate and create an incident |
| GET | `/machines/{id}/maintenance` | `maintenance` | Redirect to an external system |

## Test

From the repository root:

```sh
cargo test -p inertia-axum-example-incident-board
```

## Expected behavior

Focused tests prove deferred selection, rescued failures, scroll metadata, and
external-location responses with typed prop keys.

## Production note

Incidents, telemetry failures, flash storage, and maintenance URLs are
hard-coded fixtures. Connect these handlers to real services and durable
transient storage in production.
