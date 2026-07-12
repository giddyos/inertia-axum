#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use inertia_axum::axum::VersionLayer;
use std::hint::black_box;

fn version_layer_benchmarks(c: &mut Criterion) {
    let layer = VersionLayer::new("asset-version");
    c.bench_function("version_layer/clone_static", |b| {
        b.iter(|| black_box(layer.clone()));
    });
}

criterion_group!(benches, version_layer_benchmarks);
criterion_main!(benches);
