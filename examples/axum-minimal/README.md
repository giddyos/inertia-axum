# Axum minimal

## What this example teaches

- Build an `InertiaApp` from a Vite manifest.
- Compile an application-owned root HTML template at startup.
- Install Inertia on a normal Axum router.
- Pass `AppState` through `Router::with_state`.
- Return a small dynamic page from a named handler.

This example is intentionally minimal. Deferred props, validation, and typed
pages are introduced by the other examples.

## Important files

```text
src/main.rs                         State, handler, router, and server startup
templates/app.html                  Root document template
frontend/dist/.vite/manifest.json   Committed frontend fixture manifest
frontend/dist/assets/main.js        Minimal browser entry fixture
```

## Routes

| Method | Path | Handler | Purpose |
| --- | --- | --- | --- |
| GET | `/` | `index` | Render the Home page |

## Run

From the repository root:

```sh
cargo run -p axum-minimal
```

Open <http://127.0.0.1:3001/>.

## Expected behavior

The initial HTML response contains the Inertia root element and the Home page
data. Subsequent Inertia visits receive the JSON page response.

## Production note

The frontend build is a committed fixture, not a complete application. Use a
real Vite project and development server in production development workflows.
