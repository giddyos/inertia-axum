use criterion::{black_box, criterion_group, criterion_main, Criterion};
use inertia_axum::axum::SharedProps;

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
