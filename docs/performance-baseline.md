# Performance Baseline

Base commit: `110dcf0d345b0e78676d40d8a68661c1e271ecd3`

Phase 1 validation was rerun at refactor start on commit
`b5d433c3ff4c9ab5510c4052ae914e6dac2151ba`: formatting, workspace check,
tests (100), Clippy with warnings denied, rustdoc with warnings denied, and
benchmark compilation all passed. Criterion reports were refreshed under
`target/criterion/` for `request_context`, `page_render`, `shared_props`, and
`version_layer` before production code moves.

## Environment

- CPU: Apple M1 Max
- OS: macOS 26.5 (Darwin 25.5.0, arm64)
- Rust: rustc 1.96.0 (ac68faa20 2026-05-25)
- Profile: release / Criterion defaults (95% confidence)
- Date: 2026-07-10

## Commands

```bash
cargo bench --bench request_context
cargo bench --bench page_render
cargo bench --bench shared_props
cargo bench --bench version_layer
```

## Results

Record the complete Criterion point estimates and confidence intervals here before
changing production algorithms.

`request_context`:

| Case | Point estimate | 95% confidence interval |
| --- | ---: | --- |
| empty | 32.38 ns | 32.15–32.72 ns |
| all_headers | 1.082 µs | 1.079–1.084 µs |
| partial_data/1 | 280.29 ns | 274.23–292.59 ns |
| partial_data/8 | 628.64 ns | 613.54–676.33 ns |
| partial_data/32 | 1.7361 µs | 1.7268–1.7509 µs |
| partial_data/128 | 5.8503 µs | 5.7567–5.9190 µs |

The baseline reports above are retained for comparison with the final modular
implementation; no benchmark target is a placeholder.
