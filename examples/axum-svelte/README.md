# Axum Svelte

## What this example teaches

- Use Axum state with a typed Inertia page.
- Load one deferred prop through Svelte's `Deferred` component.
- Validate one form and display `errors.title`.
- Refresh a deferred summary explicitly after its automatic initial load.

## Important files

```text
src/main.rs                              State, typed page, handlers, and server
svelte-app/src/Pages/Todos/Index.svelte  Deferred data and validation UI
svelte-app/src/main.js                   Inertia client startup
```

## Routes

| Method | Path | Handler | Purpose |
| --- | --- | --- | --- |
| GET | `/todos` | `index` | Render the Todo page |
| POST | `/todos` | `store` | Validate and create a Todo |

## Run

From the repository root:

```sh
npm ci --prefix examples/axum-svelte/svelte-app
npm run build --prefix examples/axum-svelte/svelte-app
cargo run -p axum-svelte
```

Open <http://127.0.0.1:3002/todos>.

## Expected behavior

The seeded Todo renders immediately. Its summary shows a loading fallback and
then appears after Inertia's automatic deferred request. Submitting an empty
title redirects back and displays `Enter a todo title`; valid titles are added.

## Production note

Todos and transient validation data are stored in memory. Production apps
should persist domain state and use encrypted cookie or session-backed
transient storage.
