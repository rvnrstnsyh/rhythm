use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use poh::types::{PoH, Record};

use lib::{
    hash::{Algorithm, Hasher},
    metronome::{DEFAULT_HASHES_PER_REV, DEFAULT_US_PER_REV},
};

use criterion::{BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main};

fn hash_operations(c: &mut Criterion) {
    let mut group: BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("Hash Operations");
    let hasher: Hasher = Hasher::default();

    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    // Benchmark single hash operation.
    group.bench_function("single_hash", |b| {
        let data: [u8; 64] = [0u8; 64];
        b.iter(|| hasher.hash(black_box(&data)))
    });
    // Benchmark hash with data (event insertion).
    group.bench_function("embed_data", |b| {
        let prev_hash: [u8; 32] = [1u8; 32];
        let data: &'static [u8; 38] = b"This is an event data for benchmarking";
        b.iter(|| hasher.embed_data(black_box(&prev_hash), black_box(data)))
    });
    // Benchmark extending hash chain with different iteration counts.
    for iterations in [100, 1000, DEFAULT_HASHES_PER_REV].iter() {
        group.bench_with_input(BenchmarkId::new("extend_hash_chain", iterations), iterations, |b, &iterations| {
            let prev_hash: [u8; 32] = [2u8; 32];
            b.iter(|| hasher.extend_hash_chain(black_box(&prev_hash), black_box(iterations)))
        });
    }
    group.finish();
}

fn poh_core(c: &mut Criterion) {
    let mut group: BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("PoH Core Operations");

    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    // Benchmark PoH initialization.
    group.bench_function("poh_new", |b| {
        let seed: [u8; 64] = [b'0'; 64];
        b.iter(|| PoH::new(black_box(&seed)))
    });
    // Benchmark reving.
    group.bench_function("next_rev", |b| {
        let seed: [u8; 64] = [b'0'; 64];
        let mut poh: PoH = PoH::new(&seed);
        b.iter(|| poh.next_rev())
    });
    // Benchmark event insertion.
    group.bench_function("insert_event", |b| {
        let seed: [u8; 64] = [b'0'; 64];
        let mut poh: PoH = PoH::new(&seed);
        let event_data: &'static [u8; 38] = b"This is an event for benchmark testing";
        b.iter(|| poh.insert_event(black_box(event_data)))
    });
    group.finish();
}

// Benchmark verification operations
fn verification(c: &mut Criterion) {
    let mut group: BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("PoH Verification");
    let hasher: Hasher = Hasher::default();

    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    // Benchmark hash chain verification.
    group.bench_function("verify_hash_chain", |b| {
        let prev_hash: [u8; 32] = [3u8; 32];
        let extended: [u8; 32] = hasher.extend_hash_chain(&prev_hash, DEFAULT_HASHES_PER_REV);
        b.iter(|| hasher.verify_hash_chain(black_box(&prev_hash), black_box(&extended), black_box(DEFAULT_HASHES_PER_REV), black_box(None)))
    });
    // Benchmark hash chain verification with event data.
    group.bench_function("verify_hash_chain_with_event", |b| {
        let prev_hash: [u8; 32] = [4u8; 32];
        let event_data: &'static [u8; 37] = b"Event data for verification benchmark";
        let mut hash: [u8; 32] = hasher.embed_data(&prev_hash, event_data);

        hash = hasher.extend_hash_chain(&hash, DEFAULT_HASHES_PER_REV);
        b.iter(|| {
            hasher.verify_hash_chain(
                black_box(&prev_hash),
                black_box(&hash),
                black_box(DEFAULT_HASHES_PER_REV),
                black_box(Some(event_data)),
            )
        })
    });
    group.finish();
}

fn poh_generation(c: &mut Criterion) {
    let mut group: BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("PoH Generation");
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    // Generate a sequence of revs.
    for rev_count in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("generate_revs", rev_count), rev_count, |b, &rev_count| {
            b.iter(|| {
                let seed: [u8; 64] = [b'0'; 64];
                let mut poh: PoH = PoH::new(&seed);
                let mut records: Vec<Record> = Vec::with_capacity(rev_count as usize);

                for i in 0..rev_count {
                    let record: Record = if i % 10 == 0 {
                        // Every 10th rev, insert an event.
                        let event_data = format!("Event at rev {}", i);
                        poh.insert_event(event_data.as_bytes())
                    } else {
                        poh.next_rev()
                    };
                    records.push(record);
                }
                black_box(records)
            })
        });
    }
    group.finish();
}

// Benchmark SHA256 vs BLAKE3 hash algorithms
fn hash_algorithms(c: &mut Criterion) {
    let mut group: BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("Hash Algorithms");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));

    // Test data with different sizes.
    let test_data_small: Vec<u8> = vec![0u8; 64];
    let test_data_medium: Vec<u8> = vec![0u8; 1024];
    let test_data_large: Vec<u8> = vec![0u8; 1024 * 1024]; // 1MB.

    // First benchmark SHA256 (algorithm 0).
    let mut hasher: Hasher = Hasher::default();

    for (name, data) in [
        ("SHA-256_small", &test_data_small),
        ("SHA-256_medium", &test_data_medium),
        ("SHA-256_large", &test_data_large),
    ]
    .iter()
    {
        group.bench_function(*name, |b| b.iter(|| hasher.hash(black_box(data))));
    }

    // Then benchmark BLAKE3 (algorithm 1).
    hasher.set_algorithm(Algorithm::BLAKE3);

    for (name, data) in [
        ("BLAKE3_small", &test_data_small),
        ("BLAKE3_medium", &test_data_medium),
        ("BLAKE3_large", &test_data_large),
    ]
    .iter()
    {
        group.bench_function(*name, |b| b.iter(|| hasher.hash(black_box(data))));
    }
    // Reset to default algorithm.
    hasher.set_algorithm(Algorithm::SHA256);
    group.finish();
}

fn realtime_performance(c: &mut Criterion) {
    let mut group: BenchmarkGroup<'_, criterion::measurement::WallTime> = c.benchmark_group("Real-time Performance");
    group.warm_up_time(Duration::from_millis(500));
    // Use shorter measurement time for real-time tests.
    group.measurement_time(Duration::from_secs(2));
    // Benchmark time to perform one complete rev cycle.
    group.bench_function("rev_cycle_time", |b| {
        b.iter_custom(|iters| {
            let mut total_duration: Duration = Duration::new(0, 0);
            let seed: [u8; 64] = [b'0'; 64];

            for _ in 0..iters {
                let mut poh: PoH = PoH::new(&seed);
                let start: Instant = Instant::now();
                // Generate a rev with precise timing.
                let next_rev_target_us = DEFAULT_US_PER_REV;
                let record: Record = poh.next_rev();
                // Simulate waiting for next rev.
                let elapsed_us: u64 = start.elapsed().as_micros() as u64;

                if elapsed_us < next_rev_target_us {
                    let sleep_us: u64 = next_rev_target_us - elapsed_us;
                    std::thread::sleep(Duration::from_micros(sleep_us));
                }

                total_duration += start.elapsed();
                black_box(record);
            }
            total_duration
        })
    });
    group.finish();
}

criterion_group!(
    benches, hash_operations, poh_core, verification, poh_generation, hash_algorithms, realtime_performance,
);
criterion_main!(benches);
