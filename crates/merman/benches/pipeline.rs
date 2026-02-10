use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use merman::render::{
    LayoutOptions, SvgRenderOptions, headless_layout_options, render_layouted_svg, sanitize_svg_id,
};
use merman_core::{Engine, ParseOptions};

fn fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "flowchart",
            "flowchart TD; A[Start]-->B{Decision}; B-->|Yes|C[OK]; B-->|No|D[Cancel];",
        ),
        (
            "sequence",
            r#"sequenceDiagram
  participant Alice
  participant Bob
  Alice->>Bob: Hello
  Bob-->>Alice: Reply"#,
        ),
        (
            "state",
            r#"stateDiagram-v2
  [*] --> Still
  Still --> [*]"#,
        ),
        (
            "class",
            r#"classDiagram
  class Animal {
    +String name
  }
  class Dog
  Animal <|-- Dog"#,
        ),
    ]
}

fn bench_render_svg_sync(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();
    let layout = headless_layout_options();

    let mut group = c.benchmark_group("render_svg_sync");
    for (name, input) in fixtures() {
        let diagram_id = sanitize_svg_id(name);
        group.bench_function(name, |b| {
            b.iter_batched(
                || input,
                |text| {
                    let Some(parsed) = engine.parse_diagram_sync(text, parse_opts).unwrap() else {
                        return;
                    };
                    let diagram = merman_render::layout_parsed(&parsed, &layout).unwrap();
                    let svg_opts = SvgRenderOptions {
                        diagram_id: Some(diagram_id.clone()),
                        ..SvgRenderOptions::default()
                    };
                    let _svg =
                        render_layouted_svg(&diagram, layout.text_measurer.as_ref(), &svg_opts)
                            .unwrap();
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_parse_only_sync(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();

    let mut group = c.benchmark_group("parse_only_sync");
    for (name, input) in fixtures() {
        group.bench_function(name, |b| {
            b.iter(|| {
                let _ = engine.parse_diagram_sync(input, parse_opts).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_parse_only_known_type_sync(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();

    let mut group = c.benchmark_group("parse_only_known_type_sync");
    for (name, input) in fixtures() {
        let diagram_type = engine
            .parse_metadata_sync(input, parse_opts)
            .unwrap()
            .unwrap()
            .diagram_type;

        group.bench_function(name, |b| {
            b.iter(|| {
                let _ = engine
                    .parse_diagram_as_sync(&diagram_type, input, parse_opts)
                    .unwrap();
            });
        });
    }
    group.finish();
}

fn bench_layout_only_sync(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::default();
    let layout: LayoutOptions = headless_layout_options();

    let mut group = c.benchmark_group("layout_only_sync");
    for (name, input) in fixtures() {
        let Some(parsed) = engine.parse_diagram_sync(input, parse_opts).unwrap() else {
            continue;
        };
        group.bench_function(name, |b| {
            b.iter(|| {
                let _ = merman_render::layout_parsed(&parsed, &layout).unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse_only_sync,
    bench_parse_only_known_type_sync,
    bench_layout_only_sync,
    bench_render_svg_sync
);
criterion_main!(benches);
