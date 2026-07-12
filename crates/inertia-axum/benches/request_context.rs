#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use inertia_axum::{
    CACHE_CONTROL, PURPOSE, RequestContext, X_INERTIA, X_INERTIA_ERROR_BAG,
    X_INERTIA_EXCEPT_ONCE_PROPS, X_INERTIA_INFINITE_SCROLL_MERGE_INTENT,
    X_INERTIA_PARTIAL_COMPONENT, X_INERTIA_PARTIAL_DATA, X_INERTIA_PARTIAL_EXCEPT, X_INERTIA_RESET,
    X_INERTIA_VERSION,
};
use std::collections::HashMap;
use std::hint::black_box;

fn parse_context(headers: &HashMap<&'static str, String>) -> RequestContext {
    RequestContext::from_header_fn(|name| headers.get(name).map(String::as_str))
}

fn request_context_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_context");

    let empty = HashMap::new();
    group.bench_function("empty", |b| {
        b.iter(|| black_box(parse_context(black_box(&empty))));
    });

    let mut complete = HashMap::new();
    complete.insert(X_INERTIA, "true".to_owned());
    complete.insert(X_INERTIA_VERSION, "asset-version".to_owned());
    complete.insert(X_INERTIA_PARTIAL_COMPONENT, "Users/Index".to_owned());
    complete.insert(
        X_INERTIA_PARTIAL_DATA,
        "users,filters,permissions,companies,stats,audit".to_owned(),
    );
    complete.insert(
        X_INERTIA_PARTIAL_EXCEPT,
        "privateNotes,internalFlags".to_owned(),
    );
    complete.insert(X_INERTIA_RESET, "users,notifications".to_owned());
    complete.insert(X_INERTIA_ERROR_BAG, "createUser".to_owned());
    complete.insert(X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, "append".to_owned());
    complete.insert(
        X_INERTIA_EXCEPT_ONCE_PROPS,
        "plans,features,permissions".to_owned(),
    );
    complete.insert(PURPOSE, "prefetch".to_owned());
    complete.insert(CACHE_CONTROL, "max-age=0, no-cache".to_owned());

    group.bench_function("all_headers", |b| {
        b.iter(|| black_box(parse_context(black_box(&complete))));
    });

    for prop_count in [1_usize, 8, 32, 128] {
        let props = (0..prop_count)
            .map(|index| format!("prop{index}"))
            .collect::<Vec<_>>()
            .join(",");

        let mut headers = HashMap::new();
        headers.insert(X_INERTIA, "true".to_owned());
        headers.insert(X_INERTIA_PARTIAL_COMPONENT, "Bench".to_owned());
        headers.insert(X_INERTIA_PARTIAL_DATA, props);

        group.bench_with_input(
            BenchmarkId::new("partial_data", prop_count),
            &headers,
            |b, h| {
                b.iter(|| black_box(parse_context(black_box(h))));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, request_context_benchmarks);
criterion_main!(benches);
