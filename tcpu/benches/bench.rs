use criterion::{black_box, criterion_group, criterion_main, Criterion};

use tcpu::{Emulator, Storage, Tracer, DISK_SIZE, MEMORY_SIZE};

struct Disk {
    data: Box<[u8; DISK_SIZE]>,
}

impl Storage<[u8; DISK_SIZE]> for Disk {
    fn as_ref(&self) -> &[u8; DISK_SIZE] { &self.data }
    fn as_mut(&mut self) -> &mut [u8; DISK_SIZE] { &mut self.data }
}

fn emulator() -> Emulator<[u8; MEMORY_SIZE], Disk, impl Tracer> {
    Emulator::new([0; MEMORY_SIZE])
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("run one million cycles", |b| b.iter(|| {
        let mut emulator = emulator();
        for (i, ptr) in emulator.memory_mut().iter_mut().enumerate() {
            *ptr = i as u8;
        }
        let emulator = black_box(&mut emulator);
        emulator.run(1_000_000);
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
