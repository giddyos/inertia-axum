#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use inertia_axum::axum::SharedProps;
use std::hint::black_box;

fn shared_props_benchmarks(c: &mut Criterion) {
    let props = SharedProps::with_capacity(32)
        .value("app.name", "Inertia")
        .value("auth.user", "Ada");
    c.bench_function("shared_props/clone", |b| {
        b.iter(|| black_box(props.clone()));
    });
}

criterion_group!(benches, shared_props_benchmarks);
criterion_main!(benches);
