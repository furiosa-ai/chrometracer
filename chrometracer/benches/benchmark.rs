use criterion::{Criterion, criterion_group, criterion_main};

#[chrometracer::instrument(fields(name = test_instrument))]
fn test_instrument() {}

fn test_span() {
    chrometracer::span!(name: "test_span", is_async: false);
}

fn bench(c: &mut Criterion) {
    let _guard = chrometracer::builder().init();
    c.bench_function("instrument", |b| b.iter(|| test_instrument()));
    c.bench_function("span", |b| b.iter(|| test_span()));
}

criterion_group!(benches, bench);
criterion_main!(benches);
