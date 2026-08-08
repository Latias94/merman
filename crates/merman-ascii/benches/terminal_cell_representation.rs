//! Focused U2 prototype gate for terminal cell ownership representations.
//!
//! The pre-U2 scalar cell is frozen in the prototype module. The production terminal source is
//! included only to reuse the exact `CanvasStyle` layout without exposing benchmark-only APIs.

use criterion::{
    BatchSize, BenchmarkGroup, BenchmarkId, Criterion, Throughput, measurement::WallTime,
};
use std::env;
use std::hint::black_box;

#[path = "../../merman/benches/native_memory/allocator.rs"]
mod allocator;
#[allow(dead_code, unused_imports)]
#[path = "../src/color.rs"]
mod color;
#[path = "terminal_cell_representation/prototype.rs"]
mod prototype;
#[allow(dead_code, unused_imports)]
#[path = "../src/style_color.rs"]
mod style_color;
#[allow(dead_code, unused_imports)]
#[path = "../src/terminal.rs"]
mod terminal;

use allocator::CountingSystemAllocator;
use prototype::{
    CompactArenaSurface, CompactInternedSurface, CurrentScalarSurface, PrototypeSurface, Workload,
};

const REPORT_ENV: &str = "MERMAN_ASCII_CELL_REPORT";

#[global_allocator]
static ALLOCATOR: CountingSystemAllocator = CountingSystemAllocator::new();

fn bench_paint<S: PrototypeSurface>(group: &mut BenchmarkGroup<'_, WallTime>, workload: &Workload) {
    group.bench_function(BenchmarkId::new(S::NAME, workload.name()), |b| {
        b.iter(|| black_box(S::paint(black_box(workload))));
    });
}

fn bench_clone<S: PrototypeSurface>(group: &mut BenchmarkGroup<'_, WallTime>, workload: &Workload) {
    let surface = S::paint(workload);
    group.bench_function(BenchmarkId::new(S::NAME, workload.name()), |b| {
        b.iter(|| black_box(surface.clone()));
    });
}

fn bench_finalize<S: PrototypeSurface>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    workload: &Workload,
) {
    let surface = S::paint(workload);
    group.bench_function(BenchmarkId::new(S::NAME, workload.name()), |b| {
        b.iter(|| black_box(surface.finalize()));
    });
}

fn bench_mirror<S: PrototypeSurface>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    workload: &Workload,
) {
    let surface = S::paint(workload);
    group.bench_function(BenchmarkId::new(S::NAME, workload.name()), |b| {
        b.iter_batched(
            || surface.clone(),
            |owned| black_box(owned.mirror()),
            BatchSize::SmallInput,
        );
    });
}

fn bench_compose<S: PrototypeSurface>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    workload: &Workload,
) {
    let surface = S::paint(workload);
    group.bench_function(BenchmarkId::new(S::NAME, workload.name()), |b| {
        b.iter(|| black_box(surface.compose()));
    });
}

fn register_all_representations(
    group: &mut BenchmarkGroup<'_, WallTime>,
    workload: &Workload,
    register: impl Fn(&mut BenchmarkGroup<'_, WallTime>, &Workload, Representation),
) {
    for representation in Representation::ALL {
        register(group, workload, representation);
    }
}

#[derive(Clone, Copy)]
enum Representation {
    CurrentScalar,
    CompactArena,
    CompactInterned,
}

impl Representation {
    const ALL: [Self; 3] = [
        Self::CurrentScalar,
        Self::CompactArena,
        Self::CompactInterned,
    ];
}

fn benchmark_operation(criterion: &mut Criterion, operation: &'static str, workloads: &[Workload]) {
    let mut group = criterion.benchmark_group(format!("terminal_cell_representation/{operation}"));

    for workload in workloads {
        group.throughput(Throughput::Elements(workload.logical_graphemes() as u64));
        register_all_representations(
            &mut group,
            workload,
            |group, workload, representation| match (operation, representation) {
                ("paint", Representation::CurrentScalar) => {
                    bench_paint::<CurrentScalarSurface>(group, workload)
                }
                ("paint", Representation::CompactArena) => {
                    bench_paint::<CompactArenaSurface>(group, workload)
                }
                ("paint", Representation::CompactInterned) => {
                    bench_paint::<CompactInternedSurface>(group, workload)
                }
                ("clone", Representation::CurrentScalar) => {
                    bench_clone::<CurrentScalarSurface>(group, workload)
                }
                ("clone", Representation::CompactArena) => {
                    bench_clone::<CompactArenaSurface>(group, workload)
                }
                ("clone", Representation::CompactInterned) => {
                    bench_clone::<CompactInternedSurface>(group, workload)
                }
                ("finalize", Representation::CurrentScalar) => {
                    bench_finalize::<CurrentScalarSurface>(group, workload)
                }
                ("finalize", Representation::CompactArena) => {
                    bench_finalize::<CompactArenaSurface>(group, workload)
                }
                ("finalize", Representation::CompactInterned) => {
                    bench_finalize::<CompactInternedSurface>(group, workload)
                }
                ("mirror", Representation::CurrentScalar) => {
                    bench_mirror::<CurrentScalarSurface>(group, workload)
                }
                ("mirror", Representation::CompactArena) => {
                    bench_mirror::<CompactArenaSurface>(group, workload)
                }
                ("mirror", Representation::CompactInterned) => {
                    bench_mirror::<CompactInternedSurface>(group, workload)
                }
                ("compose", Representation::CurrentScalar) => {
                    bench_compose::<CurrentScalarSurface>(group, workload)
                }
                ("compose", Representation::CompactArena) => {
                    bench_compose::<CompactArenaSurface>(group, workload)
                }
                ("compose", Representation::CompactInterned) => {
                    bench_compose::<CompactInternedSurface>(group, workload)
                }
                _ => unreachable!("registered operation is exhaustive"),
            },
        );
    }

    group.finish();
}

fn run_criterion() {
    let workloads = prototype::workloads();
    prototype::verify_semantics(&workloads);

    let mut criterion = Criterion::default().configure_from_args();
    for operation in ["paint", "clone", "finalize", "mirror", "compose"] {
        benchmark_operation(&mut criterion, operation, &workloads);
    }
    criterion.final_summary();
}

fn main() {
    if env::var_os(REPORT_ENV).is_some() {
        prototype::print_structural_and_allocation_report(&ALLOCATOR);
    } else {
        run_criterion();
    }
}
