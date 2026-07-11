# Axum React

## What this example teaches

- Use Axum state with a typed Inertia page.
- Load one deferred prop through React's `Deferred` component.
- Validate one form and display `errors.title`.
- Refresh a deferred summary explicitly after its automatic initial load.

## Important files

```text
src/lib.rs                               State, typed page, handlers, and shared app builder
src/main.rs                              Production server startup
react-app/src/Pages/Todos/Index.jsx  Deferred data and validation UI
react-app/src/app.jsx                    Inertia client and plugin-generated SSR entry
```

## Routes

| Method | Path | Handler | Purpose |
| --- | --- | --- | --- |
| GET | `/todos` | `index` | Render the Todo page |
| POST | `/todos` | `store` | Validate and create a Todo |
| GET | `/todos/private` | `private_todos` | Render without SSR |
| GET | `/todos/preview` | `preview` | Render with conditional SSR |

## Run

From the repository root:

```sh
pnpm --dir examples/axum-react/react-app install --frozen-lockfile
pnpm --dir examples/axum-react/react-app build
cargo run -p axum-react
```

Open <http://127.0.0.1:3003/todos>.

The frontend build produces:

- `examples/axum-react/public/build/.vite/manifest.json`
- `examples/axum-react/react-app/dist/ssr/app.js`

Both are required for this production example. During Vite development,
neither production artifact is required. To clean-build and verify this
production example, run:

```sh
./scripts/test-live-ssr.sh --example-only axum-react
```

Run `./scripts/test-live-ssr.sh` to verify the managed Node lifecycle and all
three browser examples.

## Expected behavior

The seeded Todo renders immediately. Its summary shows a loading fallback and
then appears after Inertia's automatic deferred request. Submitting an empty
title redirects back and displays `Enter a todo title`; valid titles are added.

## Production note

Todos and transient validation data are stored in memory. Production apps
should persist domain state and use encrypted cookie or session-backed
transient storage.

