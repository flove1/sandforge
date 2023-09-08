use criterion::{criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("simulation");
    group.sample_size(10);

    let mut world = sandforge::bench_init();

    group.bench_function("simulation test", |b| b.iter(|| {
        sandforge::bench_fill(&mut world);
        sandforge::bench_until_empty(&mut world);
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);