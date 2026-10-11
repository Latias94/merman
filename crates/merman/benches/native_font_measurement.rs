#[path = "native_memory/allocator.rs"]
mod allocator;

use allocator::CountingSystemAllocator;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use merman::svg::export::{NativeFontContext, NativeFontLimits, NativeFontOptions};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};
use merman_render::environment::{
    HostTextMeasurementRequest, HostTextMeasurer, TextMeasurementOperation, TextMeasurementPhase,
};
use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle, WrapMode};
use serde_json::json;
use std::{hint::black_box, path::Path, thread, time::Instant};

#[global_allocator]
static ALLOCATOR: CountingSystemAllocator = CountingSystemAllocator::new();

const FLOWCHART: &str = include_str!("fixtures/flowchart_medium.mmd");
const FONT: &str = "../merman-export/tests/fonts/Cousine-Regular.ttf";
const LABEL: &str = "AV kerning, service metadata and routing labels";

fn context(cache_entries: usize) -> NativeFontContext {
    NativeFontOptions::default()
        .with_font_file(Path::new(env!("CARGO_MANIFEST_DIR")).join(FONT))
        .with_limits(NativeFontLimits {
            // Criterion can issue more operations than a single production request.
            max_measurements: usize::MAX,
            max_cache_entries: cache_entries,
            ..Default::default()
        })
        .load(&OperationControl::new())
        .expect("checked-in benchmark font must load")
}

fn native_probe(context: &NativeFontContext, text: &str, style: &TextStyle, wrap: Option<f64>) {
    black_box(
        context
            .measure(HostTextMeasurementRequest {
                operation: if wrap.is_some() {
                    TextMeasurementOperation::Wrapped
                } else {
                    TextMeasurementOperation::ComputedLength
                },
                phase: if wrap.is_some() {
                    TextMeasurementPhase::Wrap
                } else {
                    TextMeasurementPhase::ComputedLength
                },
                text,
                style,
                max_width: wrap,
                wrap_mode: WrapMode::SvgLike,
            })
            .expect("native benchmark measurement must succeed")
            .expect("native benchmark operation must be supported"),
    );
}

fn report_allocations<T>(name: &str, operation: impl FnOnce() -> T) {
    // Report one operation separately from Criterion's timing loop. These are allocator live
    // bytes, not process RSS; setup retained before the snapshot is excluded from peak growth.
    let snapshot = ALLOCATOR.begin_measurement();
    let started = Instant::now();
    let output = black_box(operation());
    let elapsed_ns = started.elapsed().as_nanos();
    let metrics = ALLOCATOR.finish_measurement(snapshot);
    drop(output);
    assert!(!metrics.counter_overflowed && !metrics.counter_underflowed);
    eprintln!(
        "{}",
        json!({
            "benchmark": name,
            "memory_scope": "single-operation-counting-system-allocator",
            "elapsed_ns": elapsed_ns,
            "snapshot_live_bytes": metrics.snapshot_live_bytes,
            "allocation_count": metrics.allocation_count,
            "allocated_bytes": metrics.allocated_bytes,
            "live_bytes_after": metrics.live_bytes_after,
            "peak_live_bytes": metrics.peak_live_bytes,
            "peak_growth_bytes": metrics.peak_growth_bytes,
        })
    );
}

fn benchmark<T>(c: &mut Criterion, name: &str, mut operation: impl FnMut() -> T) {
    report_allocations(name, &mut operation);
    c.bench_function(name, |b| b.iter(|| black_box(operation())));
}

fn benchmark_batched<I, O>(
    c: &mut Criterion,
    name: &str,
    mut setup: impl FnMut() -> I,
    mut operation: impl FnMut(I) -> O,
) {
    let input = setup();
    report_allocations(name, || operation(input));
    c.bench_function(name, |b| {
        b.iter_batched(&mut setup, &mut operation, BatchSize::PerIteration);
    });
}

fn flowchart_source(batch: usize) -> String {
    // Change every label, including groups, while retaining the fixture's graph topology.
    let diagram = FLOWCHART
        .replace("[Service ", &format!("[Batch {batch} service "))
        .replace("[Group ", &format!("[Batch {batch} group "));
    format!("---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\n{diagram}")
}

fn native_request(context: NativeFontContext) -> SvgRequest {
    SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_native_font_context(context)
            .expect("native benchmark environment"),
        ..Default::default()
    }
}

fn render_flowchart(renderer: &Renderer, source: &str, request: &SvgRequest) -> RenderOutput {
    black_box(
        renderer
            .render(RenderRequest::svg(
                source,
                OperationControl::new(),
                request.clone(),
            ))
            .expect("representative flowchart must render"),
    )
}

fn render_thread_batch(renderer: &Renderer, sources: &[String], request: &SvgRequest, jobs: usize) {
    // This includes thread creation/join, not CLI I/O, parsing arguments or publication.
    thread::scope(|scope| {
        for worker in 0..jobs {
            scope.spawn(move || {
                for source in sources.iter().skip(worker).step_by(jobs) {
                    drop(render_flowchart(renderer, source, request));
                }
            });
        }
    });
}

fn bench_native_font_measurement(c: &mut Criterion) {
    ALLOCATOR.stop_measurement();
    // Each acquisition reloads a new context. This does not flush the operating system file cache.
    benchmark(c, "native_font/context_acquisition", || context(1024));

    let native = context(1024);
    let uncached = context(0);
    let deterministic = DeterministicTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("Cousine".to_owned()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    native_probe(&native, LABEL, &style, None);
    benchmark(c, "native_font/warm_repeated/native", || {
        native_probe(&native, LABEL, &style, None);
    });
    benchmark(c, "native_font/warm_repeated/deterministic", || {
        deterministic.measure_svg_text_computed_length_px(LABEL, &style)
    });

    let labels: Vec<_> = (0..2048)
        .map(|index| format!("Unique service {index}: metadata and routing"))
        .collect();
    for (name, native_backend) in [("native", true), ("deterministic", false)] {
        let mut index = 0;
        benchmark(
            c,
            &format!("native_font/warm_uncached_unique/{name}"),
            || {
                let text = &labels[index];
                index = (index + 1) % labels.len();
                if native_backend {
                    native_probe(&uncached, text, &style, None);
                } else {
                    black_box(deterministic.measure_svg_text_computed_length_px(text, &style));
                }
            },
        );
    }

    let long_label = "Service metadata routing labels and measurements. ".repeat(64);
    native_probe(&native, &long_label, &style, Some(180.0));
    benchmark(c, "native_font/long_wrap_warm_cached/native", || {
        native_probe(&native, &long_label, &style, Some(180.0));
    });
    benchmark(c, "native_font/long_wrap_uncached/native", || {
        native_probe(&uncached, &long_label, &style, Some(180.0));
    });
    benchmark(c, "native_font/long_wrap_uncached/deterministic", || {
        deterministic.measure_wrapped(&long_label, &style, Some(180.0), WrapMode::SvgLike)
    });

    let source =
        format!("---\nconfig:\n  fontFamily: Cousine\n  htmlLabels: false\n---\n{FLOWCHART}");
    let renderer = Renderer::new();
    for (name, environment) in [
        ("deterministic", SvgEnvironment::deterministic()),
        (
            "native",
            SvgEnvironment::deterministic()
                .with_native_font_context(native.clone())
                .expect("native benchmark environment"),
        ),
    ] {
        let request = SvgRequest {
            environment,
            ..Default::default()
        };
        let mut render = || {
            renderer
                .render(RenderRequest::svg(
                    &source,
                    OperationControl::new(),
                    request.clone(),
                ))
                .expect("representative flowchart must render")
        };
        drop(render());
        benchmark(
            c,
            &format!("native_font/whole_flowchart_warm/{name}"),
            &mut render,
        );
    }

    // A fresh cache is acquired outside timing. The first render primes the reused context;
    // the timed render has wholly different labels. Reset per iteration bounds retained cache.
    let prime_source = flowchart_source(0);
    let unique_source = flowchart_source(1);
    benchmark_batched(
        c,
        "native_font/whole_flowchart_reused_context_unique/native",
        || {
            let request = native_request(context(1024));
            drop(render_flowchart(&renderer, &prime_source, &request));
            request
        },
        |request| {
            let output = render_flowchart(&renderer, &unique_source, &request);
            (request, output)
        },
    );

    // Acquisition is inside timing; source construction and the reused renderer are outside.
    // This is cold font context + SVG render, not cold process or cold OS filesystem cache.
    benchmark_batched(
        c,
        "native_font/whole_flowchart_cold_context/native",
        || (),
        |()| {
            let request = native_request(context(1024));
            let output = render_flowchart(&renderer, &unique_source, &request);
            (request, output)
        },
    );

    let saturated_labels: Vec<_> = (0..32)
        .map(|index| format!("Initial cache label {index}"))
        .collect();
    let working_set: Vec<_> = (0..8)
        .map(|index| format!("Later batch label {index}"))
        .collect();
    // Fill beyond capacity outside timing, then repeat a new working set that fits capacity.
    // The batch measures 64 probes and exposes admission-only versus replacement policies.
    benchmark_batched(
        c,
        "native_font/cache_saturated_repeated_working_set/native",
        || {
            let native = context(16);
            for label in &saturated_labels {
                native_probe(&native, label, &style, None);
            }
            native
        },
        |native| {
            for _ in 0..8 {
                for label in &working_set {
                    native_probe(&native, label, &style, None);
                }
            }
            native
        },
    );

    let batch_sources: Vec<_> = (0..8).map(flowchart_source).collect();
    for jobs in [1, 2, 4] {
        for native_backend in [true, false] {
            let backend = if native_backend {
                "native"
            } else {
                "deterministic"
            };
            benchmark_batched(
                c,
                &format!("native_font/shared_context_thread_batch_jobs_{jobs}/{backend}"),
                || {
                    if native_backend {
                        native_request(context(1024))
                    } else {
                        SvgRequest::default()
                    }
                },
                |request| {
                    render_thread_batch(&renderer, &batch_sources, &request, jobs);
                    request
                },
            );
        }
    }
}

criterion_group!(benches, bench_native_font_measurement);
criterion_main!(benches);
