//! The public SVG facade keeps font references separate from embedded font processing.

#![cfg(feature = "svg")]

use merman::diagram_theme::{DiagramThemeCompiler, ThemePreset, ThemeRecipeV1};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

#[test]
fn resource_free_cyberpunk_renders_through_the_public_facade() {
    let compiler = DiagramThemeCompiler::new();
    let theme = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let RenderOutput::Svg(Some(output)) = Renderer::new()
        .render(
            RenderRequest::svg(
                "flowchart LR\nA[Alpha] --> B[Beta]",
                OperationControl::new(),
                Default::default(),
            )
            .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("expected SVG output")
    };
    let svg = output.svg();
    assert!(svg.contains("<pattern"), "public canvas must render");
    assert!(svg.contains("<filter"), "public glow must render");
    assert!(!svg.contains("@font-face"), "no font bytes may be bundled");
}

#[test]
fn complete_spec_font_data_can_be_read_and_forwarded_without_decoding() {
    // Exchange preserves the payload; compilation owns font admission and decoding.
    let input = serde_json::json!({
        "schema_version": 1,
        "kind": "complete_spec",
        "complete_spec": {
            "assets": {
                "fonts": [{"id": "caller-font", "format": "woff2", "data_base64": "AAECAw=="}]
            }
        }
    });
    let recipe: ThemeRecipeV1 = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(recipe).unwrap(), input);
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn xml_sensitive_font_keeps_the_strict_standalone_font_admission_reason() {
    use merman::svg::{SvgPipeline, ThemePortabilityRequirement};
    use merman::{RenderError, SvgEnvironment, SvgRequest, TargetAdmissionReason};
    use merman_render::diagram_theme::{
        DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_default(
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::new(["A&B", "monospace"]).unwrap()),
                ),
            ),
        )
        .unwrap();
    let error = Renderer::new()
        .render(
            RenderRequest::svg(
                "flowchart LR\nA[Alpha] --> B[Beta]",
                OperationControl::new(),
                SvgRequest {
                    environment: SvgEnvironment::deterministic()
                        .with_theme_portability_requirement(
                            ThemePortabilityRequirement::RequirePortable,
                        ),
                    pipeline: Some(SvgPipeline::resvg_safe()),
                    ..Default::default()
                },
            )
            .with_theme(theme),
        )
        .expect_err("unembedded named fonts must retain standalone font admission");
    let RenderError::TargetAdmission(error) = error else {
        panic!("expected font admission after valid XML, got {error}");
    };
    assert!(
        error
            .receipt()
            .reasons()
            .contains(&TargetAdmissionReason::SvgFontsNotSelfContained)
    );
    assert!(
        !error
            .receipt()
            .reasons()
            .contains(&TargetAdmissionReason::SvgTerminalValidationFailed)
    );
}

#[cfg(all(feature = "all-diagrams", feature = "layout-cytoscape"))]
#[test]
fn all_38_family_fixtures_preserve_logical_font_css_after_xml_decoding() {
    use merman::svg::SvgPipeline;
    use merman::{MermaidEngine, ParseOptions, SvgEnvironment, SvgRequest};
    use merman_render::diagram_theme::{
        DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    // Reuse the frozen diagnostic corpus, including the four Railroad syntaxes.
    let fixtures = [
        (
            "flowchart_medium",
            include_str!("../benches/fixtures/flowchart_medium.mmd"),
        ),
        (
            "class_medium",
            include_str!("../benches/fixtures/class_medium.mmd"),
        ),
        (
            "state_medium",
            include_str!("../benches/fixtures/state_medium.mmd"),
        ),
        (
            "sequence_medium",
            include_str!("../benches/fixtures/sequence_medium.mmd"),
        ),
        (
            "er_medium",
            include_str!("../benches/fixtures/er_medium.mmd"),
        ),
        (
            "info_medium",
            include_str!("../benches/fixtures/info_medium.mmd"),
        ),
        (
            "pie_medium",
            include_str!("../benches/fixtures/pie_medium.mmd"),
        ),
        (
            "mindmap_medium",
            include_str!("../benches/fixtures/mindmap_medium.mmd"),
        ),
        (
            "journey_medium",
            include_str!("../benches/fixtures/journey_medium.mmd"),
        ),
        (
            "timeline_medium",
            include_str!("../benches/fixtures/timeline_medium.mmd"),
        ),
        (
            "gantt_medium",
            include_str!("../benches/fixtures/gantt_medium.mmd"),
        ),
        (
            "requirement_medium",
            include_str!("../benches/fixtures/requirement_medium.mmd"),
        ),
        (
            "gitgraph_medium",
            include_str!("../benches/fixtures/gitgraph_medium.mmd"),
        ),
        (
            "c4_medium",
            include_str!("../benches/fixtures/c4_medium.mmd"),
        ),
        (
            "sankey_medium",
            include_str!("../benches/fixtures/sankey_medium.mmd"),
        ),
        (
            "quadrant_medium",
            include_str!("../benches/fixtures/quadrant_medium.mmd"),
        ),
        (
            "zenuml_medium",
            include_str!("../benches/fixtures/zenuml_medium.mmd"),
        ),
        (
            "block_medium",
            include_str!("../benches/fixtures/block_medium.mmd"),
        ),
        (
            "packet_medium",
            include_str!("../benches/fixtures/packet_medium.mmd"),
        ),
        (
            "kanban_medium",
            include_str!("../benches/fixtures/kanban_medium.mmd"),
        ),
        (
            "architecture_medium",
            include_str!("../benches/fixtures/architecture_medium.mmd"),
        ),
        (
            "radar_medium",
            include_str!("../benches/fixtures/radar_medium.mmd"),
        ),
        (
            "treemap_medium",
            include_str!("../benches/fixtures/treemap_medium.mmd"),
        ),
        (
            "xychart_medium",
            include_str!("../benches/fixtures/xychart_medium.mmd"),
        ),
        (
            "venn_medium",
            include_str!("../benches/fixtures/venn_medium.mmd"),
        ),
        (
            "swimlane_medium",
            include_str!("../benches/fixtures/swimlane_medium.mmd"),
        ),
        (
            "eventmodeling_medium",
            include_str!("../benches/fixtures/eventmodeling_medium.mmd"),
        ),
        (
            "treeview_medium",
            include_str!("../benches/fixtures/treeview_medium.mmd"),
        ),
        (
            "ishikawa_medium",
            include_str!("../benches/fixtures/ishikawa_medium.mmd"),
        ),
        (
            "railroad_medium",
            include_str!("../benches/fixtures/railroad_medium.mmd"),
        ),
        (
            "railroad_abnf_medium",
            include_str!("../benches/fixtures/railroad_abnf_medium.mmd"),
        ),
        (
            "railroad_ebnf_medium",
            include_str!("../benches/fixtures/railroad_ebnf_medium.mmd"),
        ),
        (
            "railroad_peg_medium",
            include_str!("../benches/fixtures/railroad_peg_medium.mmd"),
        ),
        (
            "wardley_medium",
            include_str!("../benches/fixtures/wardley_medium.mmd"),
        ),
        (
            "cynefin_medium",
            include_str!("../benches/fixtures/cynefin_medium.mmd"),
        ),
        (
            "error_basic",
            include_str!("../benches/fixtures/error_basic.mmd"),
        ),
        (
            "agentflow_basic",
            include_str!("../../../fixtures/agentflow/basic.mmd"),
        ),
        (
            "usecase_basic",
            include_str!("../../../fixtures/usecase/basic.mmd"),
        ),
    ];
    assert_eq!(fixtures.len(), 38);
    let engine = MermaidEngine::new();
    let renderer = Renderer::new();
    let mut failures = Vec::new();
    for (name, source) in fixtures {
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let family = parsed.family_id().unwrap();
        for font in ["Safe Font", "A&B"] {
            let stack = FontStack::new([font, "monospace"]).unwrap();
            let expected = stack.as_css();
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_family_style(
                        family,
                        ThemeTextStyle::default().with_font_stack(stack),
                    ),
                ))
                .unwrap();
            for native in [false, true] {
                let pipeline = if native { "resvg-safe" } else { "parity" };
                let result = renderer.render(
                    RenderRequest::svg(
                        source,
                        OperationControl::new(),
                        SvgRequest {
                            environment: SvgEnvironment::deterministic(),
                            pipeline: native.then(SvgPipeline::resvg_safe),
                            ..Default::default()
                        },
                    )
                    .with_theme(theme.clone()),
                );
                let output = match result {
                    Ok(RenderOutput::Svg(Some(output))) => output,
                    other => {
                        failures.push(format!("{name}/{font}/{pipeline}: {other:?}"));
                        continue;
                    }
                };
                let document = roxmltree::Document::parse(output.svg())
                    .unwrap_or_else(|error| panic!("{name}/{font}/{pipeline}: {error}"));
                let css = document
                    .descendants()
                    .filter(|node| node.has_tag_name("style"))
                    .filter_map(|node| node.text())
                    .collect::<String>();
                match name {
                    "wardley_medium" => {
                        assert!(document.descendants().any(|node| {
                            node.attribute("font-family") == Some(expected.as_str())
                        }))
                    }
                    // The frozen Safe control has no family font consumer in these fixtures.
                    "zenuml_medium" | "agentflow_basic" | "usecase_basic" => {}
                    _ => assert!(
                        css.replace(", ", ",")
                            .contains(&expected.replace(", ", ",")),
                        "{name}/{font}/{pipeline}: XML-decoded CSS lost {expected}: {css}"
                    ),
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
