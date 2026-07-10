use criterion::{criterion_group, criterion_main, Criterion};

fn placeholder(_criterion: &mut Criterion) {}

criterion_group!(benches, placeholder);
criterion_main!(benches);
