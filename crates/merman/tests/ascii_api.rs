#![cfg(feature = "ascii")]

use merman::ascii::{
    AsciiError, AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
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

fn render_typed_model(
    model: &RenderSemanticModel,
    options: AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> Result<String, AsciiError> {
    let context = merman::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .expect("deterministic operation context");
    AsciiRenderer::new(options)?.render_model(model, &OperationControl::new(), &context, resources)
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
fn model_backend_renders_shipped_typed_models_with_caller_operation_state() {
    let options = AsciiRenderOptions::ascii();

    let class_model = render_model_for("classDiagram\nclass Animal");
    let rendered = render_typed_model(&class_model, options, AsciiResourcePolicy::default())
        .expect("class model should render");
    assert!(rendered.contains("Animal"));

    let er_model = render_model_for("erDiagram\nCUSTOMER");
    let rendered = render_typed_model(&er_model, options, AsciiResourcePolicy::default())
        .expect("ER model should render");
    assert!(rendered.contains("CUSTOMER"));

    let state_model = render_model_for("stateDiagram-v2\n[*] --> Ready");
    let rendered = render_typed_model(&state_model, options, AsciiResourcePolicy::default())
        .expect("State model should render");
    assert!(rendered.contains("Ready"));

    let xychart_model = render_model_for(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
    );
    let rendered = render_typed_model(&xychart_model, options, AsciiResourcePolicy::default())
        .expect("XYChart model should render");
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
fn canonical_ascii_renderer_applies_flowchart_node_label_wrapping() {
    let source = "flowchart TD\nA[\"Alpha Beta Gamma Delta\"]";
    let options = AsciiRenderOptions::ascii().with_flowchart_node_label_wrap_width(8);
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            source,
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

    for expected in ["Alpha", "Beta", "Gamma", "Delta"] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
    assert!(!rendered.contains("Alpha Beta Gamma Delta"), "{rendered}");
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
    let error = render_typed_model(
        &model,
        AsciiRenderOptions::ascii(),
        AsciiResourcePolicy::default(),
    )
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
        // The debug facade path keeps several fixed-size render frames alive at once. Keep the
        // constrained-stack signal while leaving enough room for those frames across toolchains.
        .stack_size(256 * 1024)
        .spawn(move || {
            let options = AsciiRenderOptions::ascii();
            let control = merman::OperationControl::new();
            let mut resources =
                AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
            resources
                .apply_limit(AsciiResourceLimitId::MaxGridCells, 10_000_000)
                .expect("valid ASCII grid override");
            let context = merman::runtime::RuntimePolicy::deterministic()
                .begin_operation()
                .expect("deterministic operation context");
            let rendered = AsciiRenderer::new(options)
                .expect("ASCII options should validate")
                .render_model(&model, &control, &context, resources)
                .expect("deep Flowchart ASCII render should not return an error");
            assert!(rendered.contains('A'));
        })
        .expect("spawn deep Flowchart ASCII render test");
    handle
        .join()
        .expect("deep Flowchart ASCII render should not overflow the stack");
}
