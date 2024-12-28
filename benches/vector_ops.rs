use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn vector_distance(c: &mut Criterion) {
    let v1: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
    let v2: Vec<f32> = vec![2.0, 3.0, 4.0, 5.0];
    
    c.bench_function("euclidean_distance", |b| {
        b.iter(|| {
            let sum: f32 = v1.iter()
                .zip(v2.iter())
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            black_box(sum.sqrt())
        })
    });
}

criterion_group!(benches, vector_distance);
criterion_main!(benches);
