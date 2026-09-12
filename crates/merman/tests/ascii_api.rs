#![cfg(feature = "ascii")]

use merman::ascii::{
    AsciiError, AsciiOutputOutcome, AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId,
    AsciiResourcePolicy, AsciiViewportPolicy, OverflowPolicy,
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
        rendered.text,
        "+---+     +---+\n|   |     |   |\n| A |---->| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn facade_report_exposes_bounded_fallback_without_remeasuring_text() {
    let output = Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(RenderRequest::ascii(
            "flowchart LR\nA[Alpha] --> B[Beta]",
            OperationControl::new(),
            AsciiRequest {
                options: AsciiRenderOptions::ascii(),
                viewport: AsciiViewportPolicy::with_max_width(5).overflow(OverflowPolicy::Fallback),
                ..Default::default()
            },
        ))
        .unwrap();
    let RenderOutput::Ascii(Some(report)) = output else {
        panic!("diagram not detected");
    };
    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert!(report.emitted_extent.width <= 5);
    let flattened = report.text.replace('\n', "");
    assert!(flattened.contains("Alpha"));
    assert!(flattened.contains("Beta"));
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
            rendered.text.contains(expected),
            "expected {expected:?} in rendered output:\n{}",
            rendered.text
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
        rendered.text,
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
            rendered.text.contains(expected),
            "missing {expected:?}:\n{}",
            rendered.text
        );
    }
    assert!(
        !rendered.text.contains("Alpha Beta Gamma Delta"),
        "{}",
        rendered.text
    );
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

    assert!(rendered.text.contains("┌"));
    assert!(rendered.text.contains("Hello"));
    assert!(rendered.text.contains("►"));
}

#[test]
fn renderer_returns_no_ascii_when_no_diagram_is_detected() {
    let error = Renderer::new()
        .with_parse_options(merman::ParseOptions::lenient())
        .render(RenderRequest::ascii(
            "this is just prose",
            OperationControl::new(),
            AsciiRequest::default(),
        ))
        .expect_err("a missing ASCII diagram should be a typed facade error");

    assert!(matches!(error, merman::RenderError::NoDiagram));
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

#[test]
fn host_recipe_preserves_typed_auto_outcomes_and_hard_failures() {
    use merman::RenderError;
    use merman::ascii::AsciiLayoutProfile;

    let source = "flowchart LR\nA[Read the complete Mermaid source] --> B[Return the complete rendered artifact]";
    let renderer = Renderer::new();
    let options = AsciiRenderOptions::unicode().with_layout_profile(AsciiLayoutProfile::Auto);
    let request = |width, overflow| AsciiRequest {
        options,
        viewport: AsciiViewportPolicy::with_max_width(width).overflow(overflow),
        ..Default::default()
    };
    for (width, overflow, expected) in [
        (80, OverflowPolicy::Error, AsciiOutputOutcome::Primary),
        (40, OverflowPolicy::Allow, AsciiOutputOutcome::WideAllowed),
        (40, OverflowPolicy::Fallback, AsciiOutputOutcome::Fallback),
    ] {
        let output = renderer
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                request(width, overflow),
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(report)) = output else {
            panic!("expected ASCII report")
        };
        assert_eq!(report.outcome, expected);
        assert_eq!(report.requested_layout_profile, AsciiLayoutProfile::Auto);
        assert_eq!(report.layout_profile, AsciiLayoutProfile::Compact);
        assert!(report.compact_attempted);
        assert!(!report.text.contains('\u{1b}'));
        let json = serde_json::to_value(report.report()).unwrap();
        assert_eq!(json["text"], report.text);
        assert_eq!(json["emitted_width"], report.emitted_extent.width);
        assert_eq!(json["outcome"], expected.as_str());
        assert_eq!(
            report.fallback.attempted,
            expected == AsciiOutputOutcome::Fallback
        );
    }
    let error = renderer
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            request(40, OverflowPolicy::Error),
        ))
        .unwrap_err();
    assert!(matches!(
        error,
        RenderError::Ascii(AsciiError::WidthOverflow { max_width: 40, .. })
    ));

    let cancelled = OperationControl::new();
    cancelled.cancel();
    let error = renderer
        .render(RenderRequest::ascii(
            source,
            cancelled,
            request(40, OverflowPolicy::Fallback),
        ))
        .unwrap_err();
    assert!(matches!(error, RenderError::Cancelled(_)));

    let mut limited = request(40, OverflowPolicy::Fallback);
    limited
        .resources
        .apply_limit(AsciiResourceLimitId::MaxGridCells, 1)
        .unwrap();
    let error = renderer
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            limited,
        ))
        .unwrap_err();
    assert!(matches!(error, RenderError::ResourceLimitExceeded(_)));
}

#[test]
fn terminal_palette_recipe_preserves_geometry_and_rejects_styled_fallback() {
    use merman::RenderError;
    use merman::ascii::{AsciiColorMode, AsciiColorTheme, AsciiRgb, AsciiTerminalPalette};

    let source = "flowchart LR\nA[Host] --> B[Text]";
    let renderer = Renderer::new();
    let theme = AsciiColorTheme::from_terminal_palette(AsciiTerminalPalette::new(
        AsciiRgb::new(229, 231, 235),
        AsciiRgb::new(15, 23, 42),
    ));
    let mut plain = None;
    for mode in [AsciiColorMode::Plain, AsciiColorMode::TrueColor] {
        let options = AsciiRenderOptions::unicode()
            .with_color_mode(mode)
            .with_color_theme(theme);
        let output = renderer
            .render(RenderRequest::ascii(
                source,
                OperationControl::new(),
                AsciiRequest {
                    options,
                    viewport: AsciiViewportPolicy::with_max_width(80)
                        .overflow(OverflowPolicy::Error),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(report)) = output else {
            panic!("expected ASCII report")
        };
        if mode == AsciiColorMode::Plain {
            assert!(!report.text.contains('\u{1b}'));
            plain = Some(report.emitted_extent);
        } else {
            assert_eq!(Some(report.emitted_extent), plain);
            assert_eq!(report.metadata().encoding, "truecolor");
            assert!(report.text.contains("\u{1b}[38;2;229;231;235m"));
            let error = renderer
                .render(RenderRequest::ascii(
                    source,
                    OperationControl::new(),
                    AsciiRequest {
                        options,
                        viewport: AsciiViewportPolicy::with_max_width(80)
                            .overflow(OverflowPolicy::Fallback),
                        ..Default::default()
                    },
                ))
                .unwrap_err();
            assert!(matches!(
                error,
                RenderError::Ascii(AsciiError::InvalidOption {
                    field: "ascii_viewport.overflow",
                    ..
                })
            ));
        }
    }
}

#[test]
fn explicit_ascii_request_is_independent_of_terminal_environment() {
    const CHILD_FLAG: &str = "MERMAN_TEST_TERMINAL_ENV_CHILD";
    const MARKER: &str = "MERMAN_HOST_REPORT:";
    if std::env::var_os(CHILD_FLAG).is_some() {
        let output = Renderer::new()
            .render(RenderRequest::ascii(
                "flowchart LR\nA[Host] --> B[Artifact]",
                OperationControl::new(),
                AsciiRequest {
                    options: AsciiRenderOptions::unicode(),
                    viewport: AsciiViewportPolicy::with_max_width(80),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::Ascii(Some(report)) = output else {
            panic!("expected ASCII report")
        };
        assert!(!report.text.contains('\u{1b}'));
        println!(
            "\n{MARKER}{}",
            serde_json::to_string(&report.report()).unwrap()
        );
        return;
    }

    // Configure child environments instead of mutating process-global state in parallel tests.
    let environments = [
        vec![],
        vec![
            ("TERM", "dumb"),
            ("NO_COLOR", "1"),
            ("COLUMNS", "10"),
            ("LINES", "1"),
        ],
        vec![
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("CLICOLOR_FORCE", "1"),
            ("COLUMNS", "200"),
        ],
    ];
    let mut reports = Vec::new();
    for environment in environments {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "explicit_ascii_request_is_independent_of_terminal_environment",
                "--nocapture",
            ])
            .env(CHILD_FLAG, "1");
        for key in [
            "TERM",
            "NO_COLOR",
            "COLORTERM",
            "CLICOLOR",
            "CLICOLOR_FORCE",
            "COLUMNS",
            "LINES",
        ] {
            command.env_remove(key);
        }
        command.envs(environment);
        let output = command.output().unwrap();
        assert!(output.status.success(), "child failed: {:?}", output);
        let stdout = String::from_utf8(output.stdout).unwrap();
        let report = stdout
            .lines()
            .find_map(|line| line.strip_prefix(MARKER))
            .expect("child must emit its report");
        reports.push(serde_json::from_str::<serde_json::Value>(report).unwrap());
    }
    assert_eq!(reports[0], reports[1]);
    assert_eq!(reports[0], reports[2]);
}
