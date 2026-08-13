#![cfg(feature = "ascii")]

use merman::ascii::{
    AsciiError, AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy,
    HeadlessAsciiRenderer, render_ascii_sync, render_class, render_er, render_model,
    render_model_with_operation, render_state, render_xychart,
};
use merman::resources::ResourceProfile;
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

    let RenderSemanticModel::Class(class_model) = render_model_for("classDiagram\nclass Animal")
    else {
        panic!("expected class render model");
    };
    let rendered = render_class(&class_model, &options).unwrap();
    assert!(rendered.contains("Animal"));

    let RenderSemanticModel::Er(er_model) = render_model_for("erDiagram\nCUSTOMER") else {
        panic!("expected ER render model");
    };
    let rendered = render_er(&er_model, &options).unwrap();
    assert!(rendered.contains("CUSTOMER"));

    let RenderSemanticModel::State(state_model) =
        render_model_for("stateDiagram-v2\n[*] --> Ready")
    else {
        panic!("expected State render model");
    };
    let rendered = render_state(&state_model, &options).unwrap();
    assert!(rendered.contains("Ready"));

    let RenderSemanticModel::XyChart(xychart_model) = render_model_for(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
    ) else {
        panic!("expected XYChart render model");
    };
    let rendered = render_xychart(&xychart_model, &options).unwrap();
    assert!(rendered.contains("###"));
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
fn public_ascii_renderers_apply_flowchart_node_label_wrapping() {
    let source = "flowchart TD\nA[\"Alpha Beta Gamma Delta\"]";
    let options = AsciiRenderOptions::ascii().with_flowchart_node_label_wrap_width(8);
    let engine = merman::Engine::new();
    let direct = render_ascii_sync(&engine, source, merman::ParseOptions::strict(), &options)
        .unwrap()
        .unwrap();
    let headless = HeadlessAsciiRenderer::new()
        .with_strict_parsing()
        .with_ascii_options(options)
        .render_ascii_sync(source)
        .unwrap()
        .unwrap();

    assert_eq!(headless, direct);
    for expected in ["Alpha", "Beta", "Gamma", "Delta"] {
        assert!(direct.contains(expected), "missing {expected:?}:\n{direct}");
    }
    assert!(!direct.contains("Alpha Beta Gamma Delta"), "{direct}");
}

#[test]
fn renderer_renders_sequence_with_unicode_defaults() {
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
    let error = render_model(&model, &AsciiRenderOptions::ascii())
        .expect_err("the Interactive profile should reject nesting beyond its public limit");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxNestingDepth
                && details.actual == 257
                && details.max == 256
    ));

    let handle = std::thread::Builder::new()
        .name("ascii-deep-flowchart-subgraph".to_string())
        .stack_size(64 * 1024)
        .spawn(move || {
            let options = AsciiRenderOptions::ascii();
            let mut resources =
                AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
            resources
                .apply_limit(AsciiResourceLimitId::MaxGridCells, 10_000_000)
                .expect("valid ASCII grid override");
            let context = merman::runtime::RuntimePolicy::deterministic()
                .begin_operation()
                .expect("deterministic operation context");
            let rendered = render_model_with_operation(
                &model,
                &options,
                &merman::OperationControl::new(),
                &context,
                resources,
            )
            .expect("deep Flowchart ASCII render should not return an error");
            assert!(rendered.contains('A'));
        })
        .expect("spawn deep Flowchart ASCII render test");
    handle
        .join()
        .expect("deep Flowchart ASCII render should not overflow the stack");
}
