use criterion::{criterion_group, criterion_main, Criterion};
use sandforge::bench;

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("simulation");
    group.sample_size(10);

    group.bench_function("simulation test", |b| b.iter(|| bench()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);