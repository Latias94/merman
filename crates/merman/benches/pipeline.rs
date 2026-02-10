use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use merman::render::{
    LayoutOptions, SvgRenderOptions, headless_layout_options, render_layouted_svg,
};
use merman_core::{Engine, ParseOptions};
use std::hint::black_box;

fn fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "flowchart_tiny",
            include_str!("fixtures/flowchart_tiny.mmd"),
        ),
        ("sequence_tiny", include_str!("fixtures/sequence_tiny.mmd")),
        ("state_tiny", include_str!("fixtures/state_tiny.mmd")),
        ("class_tiny", include_str!("fixtures/class_tiny.mmd")),
    ]
}

fn layout_size(layout: &merman_render::model::LayoutDiagram) -> usize {
    use merman_render::model::LayoutDiagram;
    match layout {
        // These are the only variants exercised by this bench's fixture set today.
        LayoutDiagram::FlowchartV2(v) => v.nodes.len() + v.edges.len() + v.clusters.len(),
        LayoutDiagram::SequenceDiagram(v) => v.nodes.len() + v.edges.len() + v.clusters.len(),
        LayoutDiagram::StateDiagramV2(v) => v.nodes.len() + v.edges.len() + v.clusters.len(),
        LayoutDiagram::ClassDiagramV2(v) => v.nodes.len() + v.edges.len() + v.clusters.len(),
        _ => 0,
    }
}

fn bench_parse(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();

    let mut group = c.benchmark_group("parse");
    for (name, input) in fixtures() {
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let parsed = engine
                    .parse_diagram_sync(black_box(data), parse_opts)
                    .unwrap();
                black_box(parsed.is_some());
            })
        });
    }
    group.finish();
}

fn bench_parse_known_type(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();

    let mut group = c.benchmark_group("parse_known_type");
    for (name, input) in fixtures() {
        let diagram_type = engine
            .parse_metadata_sync(input, parse_opts)
            .unwrap()
            .expect("fixture must be a diagram")
            .diagram_type;

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let parsed = engine
                    .parse_diagram_as_sync(&diagram_type, black_box(data), parse_opts)
                    .unwrap();
                black_box(parsed.is_some());
            })
        });
    }
    group.finish();
}

fn bench_layout(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();
    let layout: LayoutOptions = headless_layout_options();

    let mut group = c.benchmark_group("layout");
    for (name, input) in fixtures() {
        let Some(parsed) = engine.parse_diagram_sync(input, parse_opts).unwrap() else {
            continue;
        };

        group.bench_with_input(BenchmarkId::from_parameter(name), &parsed, |b, data| {
            b.iter(|| {
                let diagram = merman_render::layout_parsed(black_box(data), &layout).unwrap();
                black_box(layout_size(&diagram.layout));
            })
        });
    }
    group.finish();
}

fn bench_render(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();
    let layout: LayoutOptions = headless_layout_options();

    let mut group = c.benchmark_group("render");
    for (name, input) in fixtures() {
        let Some(parsed) = engine.parse_diagram_sync(input, parse_opts).unwrap() else {
            continue;
        };
        let diagram = merman_render::layout_parsed(&parsed, &layout).unwrap();

        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::render::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };

        group.bench_with_input(BenchmarkId::from_parameter(name), &diagram, |b, data| {
            b.iter(|| {
                let svg =
                    render_layouted_svg(black_box(data), layout.text_measurer.as_ref(), &svg_opts)
                        .unwrap();
                black_box(svg.len());
            })
        });
    }
    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();
    let layout = headless_layout_options();

    let mut group = c.benchmark_group("end_to_end");
    for (name, input) in fixtures() {
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::render::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter_batched(
                || data,
                |text| {
                    let Some(parsed) = engine
                        .parse_diagram_sync(black_box(text), parse_opts)
                        .unwrap()
                    else {
                        return;
                    };
                    let diagram = merman_render::layout_parsed(&parsed, &layout).unwrap();
                    let svg =
                        render_layouted_svg(&diagram, layout.text_measurer.as_ref(), &svg_opts)
                            .unwrap();
                    black_box(svg.len());
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_parse_known_type,
    bench_layout,
    bench_render,
    bench_end_to_end
);
criterion_main!(benches);
