#![cfg(feature = "ascii")]

use merman::ascii::{
    AsciiRenderOptions, AsciiResourcePolicy, render_model_with_local_time_zone,
    render_model_with_operation,
};
use merman::{
    AsciiRequest, OperationControl, RenderOutput, RenderRequest, RenderSemanticModel, Renderer,
};

fn render_model_for(source: &str) -> RenderSemanticModel {
    merman::Engine::new()
        .parse_diagram_for_render_model_sync(source, merman::ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_parts()
        .1
}

fn deeply_nested_flowchart(depth: usize) -> String {
    let mut lines = vec!["flowchart TB".to_string()];
    for i in 0..depth {
        lines.push(format!("subgraph n{i}"));
    }
    lines.push("A".to_string());
    for _ in 0..depth {
        lines.push("end".to_string());
    }
    lines.join("\n")
}

#[test]
fn renderer_renders_ascii_flowchart_from_mermaid_text() {
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            "flowchart LR\nA --> B",
            OperationControl::new(),
            AsciiRequest {
                options: AsciiRenderOptions::ascii(),
                ..Default::default()
            },
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(rendered)) = output else {
        panic!("diagram not detected");
    };

    assert_eq!(
        rendered,
        "+---+     +---+\n|   |     |   |\n| A |---->| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn renderer_renders_shipped_ascii_reference_diagram_families() {
    let cases = [
        ("classDiagram\nclass Animal", "Animal"),
        ("erDiagram\nCUSTOMER", "CUSTOMER"),
        (
            r#"xychart
title "Sales"
x-axis [Jan, Feb]
y-axis 0 --> 10
bar [2, 8]
"#,
            "Sales",
        ),
    ];

    for (source, expected) in cases {
        let output = Renderer::new()
            .with_parse_options(merman::ParseOptions::strict())
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                AsciiRequest {
                    options: AsciiRenderOptions::ascii(),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(rendered)) = output else {
            panic!("diagram not detected");
        };

        assert!(
            rendered.contains(expected),
            "expected {expected:?} in rendered output:\n{rendered}"
        );
    }
}

#[test]
fn direct_ascii_exports_render_shipped_typed_models() {
    let options = AsciiRenderOptions::ascii();

    for (source, expected) in [
        ("classDiagram\nclass Animal", "Animal"),
        ("erDiagram\nCUSTOMER", "CUSTOMER"),
        (
            r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
            "###",
        ),
    ] {
        let model = render_model_for(source);
        let rendered = render_model_with_local_time_zone(
            &model,
            &options,
            &merman::time::LocalTimeZone::utc(),
        )
        .unwrap();
        assert!(rendered.contains(expected));
    }
}

#[test]
fn renderer_uses_ascii_options_for_padding() {
    let mut options = AsciiRenderOptions::ascii();
    options.graph_padding_x = 2;
    options.graph_padding_y = 1;
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            "graph LR\nA --> B",
            OperationControl::new(),
            AsciiRequest {
                options,
                ..Default::default()
            },
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(rendered)) = output else {
        panic!("diagram not detected");
    };

    assert_eq!(
        rendered,
        "+---+  +---+\n|   |  |   |\n| A |->| B |\n|   |  |   |\n+---+  +---+\n"
    );
}

#[test]
fn headless_ascii_renderer_renders_sequence_with_unicode_defaults() {
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello",
            OperationControl::new(),
            AsciiRequest::default(),
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(rendered)) = output else {
        panic!("diagram not detected");
    };

    assert!(rendered.contains("┌"));
    assert!(rendered.contains("Hello"));
    assert!(rendered.contains("►"));
}

#[test]
fn renderer_returns_no_ascii_when_no_diagram_is_detected() {
    let rendered = Renderer::new()
        .with_parse_options(merman::ParseOptions::lenient())
        .render(RenderRequest::ascii(
            "this is just prose",
            OperationControl::new(),
            AsciiRequest::default(),
        ))
        .unwrap();

    assert!(matches!(rendered, RenderOutput::Ascii(None)));
}

#[test]
fn render_ascii_model_handles_deep_flowchart_subgraph_chain_with_small_stack() {
    const DEPTH: usize = 512;
    let source = deeply_nested_flowchart(DEPTH);
    let model = render_model_for(&source);
    let handle = std::thread::Builder::new()
        .name("ascii-deep-flowchart-subgraph".to_string())
        .stack_size(64 * 1024)
        .spawn(move || {
            let options = AsciiRenderOptions::ascii();
            let context = merman::runtime::RuntimePolicy::deterministic()
                .begin_operation()
                .expect("deterministic operation context");
            let rendered = render_model_with_operation(
                &model,
                &options,
                &merman::OperationControl::new(),
                &context,
                AsciiResourcePolicy::with_max_grid_cells(10_000_000),
            )
            .expect("deep Flowchart ASCII render should not return an error");
            assert!(rendered.contains('A'));
        })
        .expect("spawn deep Flowchart ASCII render test");
    handle
        .join()
        .expect("deep Flowchart ASCII render should not overflow the stack");
}
