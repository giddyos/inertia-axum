use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use inertia_axum::{Page, PageMetadata};
use serde_json::json;

fn page_render_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_render");
    for size in [1024_usize, 64 * 1024, 1024 * 1024] {
        let page = Page::from_parts(
            "Bench",
            json!({"text": "x".repeat(size)}),
            "/bench",
            None,
            PageMetadata::new(),
        );
        group.bench_with_input(BenchmarkId::new("ordinary", size), &page, |b, page| {
            b.iter(|| black_box(serde_json::to_string(black_box(page)).unwrap()));
        });
    }
    let script_page = Page::from_parts(
        "Bench",
        json!({"text": "</script>".repeat(8192)}),
        "/bench",
        None,
        PageMetadata::new(),
    );
    group.bench_function("script_sensitive_64k", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&script_page)).unwrap()));
    });
    group.finish();
}

criterion_group!(benches, page_render_benchmarks);
criterion_main!(benches);
