use criterion::{black_box, criterion_group, criterion_main, Criterion};

use tcpu::{DiskMemory, Emulator, Memory, Tracer};

fn emulator() -> Emulator<Memory, DiskMemory, impl Tracer> {
    Emulator::new()
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("run 1e5 cycles", |b| b.iter(|| {
        let mut emulator = emulator();
        for (i, ptr) in emulator.memory_mut().iter_mut().enumerate() {
            *ptr = i as u8;
        }
        let emulator = black_box(&mut emulator);
        emulator.run(100_000);
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
