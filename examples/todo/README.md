# Typed Todo application

## What this example teaches

- Structure Inertia around normal Axum state and named handlers.
- Return a typed page with `todos` and deferred `stats`.
- Validate a form through `Validated<CreateTodo>`.
- Test partial props and redirect errors in process.

## Important files

```text
src/lib.rs   Axum state, handlers, router, typed page, and tests
```

## Routes

| Method | Path | Handler | Purpose |
| --- | --- | --- | --- |
| GET | `/todos` | `index` | Render the Todo page |
| POST | `/todos` | `store` | Validate and create a Todo |

## Test

From the repository root:

```sh
cargo test -p inertia-axum-example-todo
```

## Expected behavior

The tests prove that `stats` is advertised but omitted initially, a typed
partial request loads it without `todos`, and invalid input redirects back with
an error in the normal `errors` prop. This is an in-process application example,
not a browser server.

## Production note

The Todo repository and `MemoryTransient` store are in-memory fixtures. Use
persistent application storage and encrypted cookie or session-backed transient
storage in production.
