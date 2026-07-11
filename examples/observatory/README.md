# Observatory protocol fixture

This is a protocol regression fixture for less common behavior, not a
recommended starting point.

## What this example teaches

- Once props.
- Deep merge metadata.
- Scroll prepend and reset behavior.
- Redacted old input.
- Rescued prop failures.

## Important files

```text
src/lib.rs   Named handlers, protocol policies, and regression tests
```

## Routes

| Method | Path | Handler | Purpose |
| --- | --- | --- | --- |
| GET | `/anomalies/{id}` | `show` | Render the protocol fixture page |
| POST | `/anomalies` | `store` | Exercise validation and old input |
| GET | `/telescopes/{id}/console` | `console` | Return an external location |

## Test

From the repository root:

```sh
cargo test -p inertia-axum-example-observatory
```

## Expected behavior

The tests verify advanced page metadata, selective resolver execution,
prepend/reset requests, rescued failures, and redaction of sensitive old input.

## Production note

All records, failures, counters, external URLs, and transient state are fixture
data. They are intentionally deterministic for protocol regression tests.
