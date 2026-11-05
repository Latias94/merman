//! The public SVG facade keeps font references separate from embedded font processing.

#![cfg(feature = "svg")]

use merman::diagram_theme::{DiagramThemeCompiler, ThemePreset, ThemeRecipeV1};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

#[cfg(all(feature = "all-diagrams", feature = "layout-cytoscape"))]
#[path = "support/theme_family_corpus.rs"]
mod theme_family_corpus;

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
fn all_render_families_preserve_logical_font_css_after_xml_decoding() {
    use std::collections::BTreeSet;

    use merman::svg::SvgPipeline;
    use merman::{
        MermaidEngine, ParseOptions, SvgEnvironment, SvgRequest, diagram_family_capabilities,
    };
    use merman_render::diagram_theme::{
        DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    let fixtures = theme_family_corpus::FIXTURES;
    let expected_families: BTreeSet<_> = diagram_family_capabilities()
        .iter()
        .filter(|capability| capability.has_render_parser)
        .map(|capability| capability.family_id)
        .collect();
    let mut represented_families = BTreeSet::new();
    let engine = MermaidEngine::new();
    let renderer = Renderer::new();
    let mut failures = Vec::new();
    for &(name, source) in fixtures {
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let family = parsed.family_id().unwrap();
        represented_families.insert(family);
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
                    "usecase_basic" => {
                        let labels: Vec<_> = document
                            .descendants()
                            .filter(|node| {
                                (node.has_tag_name("text") || node.has_tag_name("p"))
                                    && node.descendants().any(|child| {
                                        child.is_text()
                                            && child
                                                .text()
                                                .is_some_and(|text| !text.trim().is_empty())
                                    })
                            })
                            .collect();
                        assert!(
                            !labels.is_empty(),
                            "{name}/{font}/{pipeline}: no visible labels"
                        );
                        for label in labels {
                            let family = label.ancestors().find_map(|ancestor| {
                                ancestor
                                    .attribute("style")?
                                    .split(';')
                                    .find_map(|declaration| {
                                        let (property, value) = declaration.split_once(':')?;
                                        (property.trim() == "font-family").then(|| {
                                            value.trim().trim_end_matches("!important").trim()
                                        })
                                    })
                            });
                            assert_eq!(
                                family,
                                Some(expected.as_str()),
                                "{name}/{font}/{pipeline}: label font must reach actual glyph ancestors"
                            );
                        }
                    }
                    // The frozen Safe control has no family font consumer in these fixtures.
                    "zenuml_medium" => {}
                    _ => assert!(
                        css.replace(", ", ",")
                            .contains(&expected.replace(", ", ",")),
                        "{name}/{font}/{pipeline}: XML-decoded CSS lost {expected}: {css}"
                    ),
                }
            }
        }
    }
    assert_eq!(
        represented_families, expected_families,
        "every registered render family needs a font exchange fixture"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
