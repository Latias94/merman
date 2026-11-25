#[path = "../support/font_workloads.rs"]
mod font_workloads;

use criterion::{BatchSize, Criterion};
use font_workloads::Observer;
use std::hint::black_box;

struct Timer<'a>(&'a mut Criterion);

impl Observer for Timer<'_> {
    fn single<T>(&mut self, name: &str, mut operation: impl FnMut() -> T) {
        drop(black_box(operation()));
        self.0
            .bench_function(name, |b| b.iter(|| black_box(operation())));
    }

    fn batched<I, T>(
        &mut self,
        name: &str,
        mut setup: impl FnMut() -> I,
        mut operation: impl FnMut(I) -> T,
    ) {
        drop(black_box(operation(setup())));
        self.0.bench_function(name, |b| {
            b.iter_batched(&mut setup, &mut operation, BatchSize::PerIteration);
        });
    }
}

pub(super) fn register(criterion: &mut Criterion) {
    font_workloads::observe(&mut Timer(criterion));
}
