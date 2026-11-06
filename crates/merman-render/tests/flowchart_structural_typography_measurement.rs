#![cfg(feature = "diagram-flowchart")]

//! Structural source fonts must own measurement as well as terminal style and evidence.

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, Specified,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family::{self, RenderedFamilySvg};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

mod common;

const LAYOUTS: &[&str] = &[
    "dagre",
    #[cfg(feature = "layout-elk")]
    "elk",
];

fn theme(font_stack: bool, font_size: bool) -> DiagramTheme {
    let mut rules = ThemeRuleSet::default();
    if font_stack {
        let mut patch = ThemeStylePatch::default();
        patch.typography.font_stack = Specified::Value(FontStack::single("monospace").unwrap());
        rules = rules.with_rule(ThemeRule::new(ThemeTarget::NodeLabel, patch));
    }
    if font_size {
        let mut patch = ThemeStylePatch::default();
        patch.typography.font_size_px = Specified::Value(24.0);
        rules = rules.with_rule(ThemeRule::new(ThemeTarget::NodeLabel, patch));
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(rules))
        .unwrap()
}

fn render(source: &str, theme: Option<&DiagramTheme>, layout: &str) -> RenderedFamilySvg {
    render_mode(source, theme, layout, true)
}

fn render_mode(
    source: &str,
    theme: Option<&DiagramTheme>,
    layout: &str,
    html: bool,
) -> RenderedFamilySvg {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(json!({
            "layout": layout, "look": "classic", "htmlLabels": html,
            // Isolate glyph measurement from Mermaid's default 120px minimum label width.
            "flowchart": {"htmlLabels": html, "minNodeWidth": 0}
        })),
    );
    let engine = match theme {
        Some(theme) => common::install_parse_compatibility(theme, engine),
        None => engine,
    };
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort);
    let session = match theme {
        Some(theme) => environment.begin_session_with_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("structural-font-measurement".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
}

fn label_size(svg: &str) -> (f64, f64) {
    label_size_for(svg, "A")
}

fn label_size_for(svg: &str, id: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let node = document
        .descendants()
        .find(|node| node.attribute("data-id") == Some(id))
        .unwrap();
    let label = node
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .unwrap();
    (
        label.attribute("width").unwrap().parse().unwrap(),
        label.attribute("height").unwrap().parse().unwrap(),
    )
}

#[test]
fn collapsed_subgraph_structural_fonts_match_the_measured_source_size() {
    let empty = theme(false, false);
    let typed = theme(false, true);
    let diagram = "flowchart LR\nsubgraph G[WIDE WIDE]\nA\nend\nG@{ view: collapsed }\n";
    for layout in LAYOUTS {
        let expected = render(&format!("{diagram}style G font-size:32px\n"), None, layout);
        let expected_size = label_size_for(expected.svg(), "G");
        for class in ["root", "nodes", "label", "nodeLabel"] {
            let source = format!("{diagram}classDef {class} font-size:32px\n");
            for (name, theme) in [
                ("source", None),
                ("empty", Some(&empty)),
                ("typed", Some(&typed)),
            ] {
                let rendered = render(&source, theme, layout);
                assert_eq!(
                    label_size_for(rendered.svg(), "G"),
                    expected_size,
                    "{layout}, {class}, {name}: the collapsed label cache must use the source size"
                );
                if name == "typed" {
                    let completion = rendered.into_completion();
                    let evidence = merman_render::__private::family_evidence(completion.report());
                    assert_eq!(evidence.applied_count(), 0, "{layout}, {class}");
                    assert_eq!(evidence.not_applicable_count(), 1, "{layout}, {class}");
                    assert_eq!(evidence.theme_residual_count(), 0, "{layout}, {class}");
                }
            }
        }
    }
}

#[test]
fn absolute_structural_size_matches_source_measurement_with_or_without_a_theme() {
    let empty = theme(false, false);
    let typed = theme(false, true);
    for layout in LAYOUTS {
        let expected = render(
            "flowchart LR\nA[WIDE]\nstyle A font-size:32px\n",
            None,
            layout,
        );
        let smaller = render(
            "flowchart LR\nA[WIDE]\nstyle A font-size:24px\n",
            None,
            layout,
        );
        let expected_size = label_size(expected.svg());
        let smaller_size = label_size(smaller.svg());
        assert!(
            expected_size.0 > smaller_size.0,
            "{layout}: source width must use 32px: expected={expected_size:?}, smaller={smaller_size:?}"
        );
        assert!(
            expected_size.1 > smaller_size.1,
            "{layout}: source height must use 32px: expected={expected_size:?}, smaller={smaller_size:?}"
        );
        for class in ["root", "nodes", "label", "nodeLabel"] {
            let source = format!("flowchart LR\nA[WIDE]\nclassDef {class} font-size:32px\n");
            let baseline = render(&source, None, layout);
            let empty = render(&source, Some(&empty), layout);
            let rendered = render(&source, Some(&typed), layout);
            assert_eq!(
                label_size(baseline.svg()),
                expected_size,
                "{layout}, {class}"
            );
            assert_eq!(empty.svg(), baseline.svg(), "{layout}, {class}");
            assert_eq!(rendered.svg(), baseline.svg(), "{layout}, {class}");
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn structural_font_ownership_keeps_the_other_typed_font_in_measurement() {
    let typed = theme(true, true);
    for layout in LAYOUTS {
        for (property, value) in [("font-family", "serif"), ("font-size", "32px")] {
            let inline = format!("flowchart LR\nA[WIDE]\nstyle A {property}:{value}\n");
            let expected = render(&inline, Some(&typed), layout);
            for class in ["root", "nodes"] {
                let source =
                    format!("flowchart LR\nA[WIDE]\nclassDef {class} {property}:{value}\n");
                let rendered = render(&source, Some(&typed), layout);
                assert_eq!(
                    label_size(rendered.svg()),
                    label_size(expected.svg()),
                    "{layout}, {class}, {property}"
                );
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn unresolved_structural_size_keeps_source_residual_and_never_measures_the_typed_size() {
    let empty = theme(false, false);
    let typed = theme(false, true);
    for layout in LAYOUTS {
        for value in ["2em", "var(--source-size)"] {
            let source = format!("flowchart LR\nA[WIDE]\nclassDef nodes font-size:{value}\n");
            let baseline = render(&source, None, layout);
            let empty = render(&source, Some(&empty), layout);
            let rendered = render(&source, Some(&typed), layout);
            assert_eq!(empty.svg(), baseline.svg(), "{layout}, {value}");
            assert_eq!(
                label_size(rendered.svg()),
                label_size(baseline.svg()),
                "{layout}, {value}"
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
            assert!(evidence.source_residual_count() > 0, "{layout}, {value}");
        }
    }
}

#[test]
fn structural_font_shorthand_protects_both_facets_without_inventing_measurement() {
    let empty = theme(false, false);
    let typed = theme(true, true);
    for layout in LAYOUTS {
        for class in ["root", "nodes", "label", "nodeLabel"] {
            for value in ["italic 32px serif", "var(--source-font)"] {
                let source = format!("flowchart LR\nA[WIDE]\nclassDef {class} font:{value}\n");
                let baseline = render(&source, None, layout);
                let empty = render(&source, Some(&empty), layout);
                let rendered = render(&source, Some(&typed), layout);
                assert_eq!(empty.svg(), baseline.svg(), "{layout}, {class}, {value}");
                assert_eq!(
                    label_size(rendered.svg()),
                    label_size(baseline.svg()),
                    "{layout}, {class}, {value}"
                );
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.applied_count(), 0, "{layout}, {class}, {value}");
                assert_eq!(evidence.not_applicable_count(), 2);
                assert_eq!(evidence.theme_residual_count(), 0);
                assert!(evidence.source_residual_count() > 0);
            }
        }
    }
}

#[test]
fn bpmn_label_wrappers_own_html_fonts_but_body_and_leaf_classes_do_not() {
    fn assert_bpmn_start_fixture(svg: &str) {
        let document = roxmltree::Document::parse(svg).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("data-id") == Some("A"))
            .unwrap();
        let classes = node.attribute("class").unwrap();
        for expected in ["bpmn-event", "bpmn-event-start"] {
            assert!(
                classes
                    .split_ascii_whitespace()
                    .any(|class| class == expected),
                "fixture must render a BPMN start event, found classes: {classes}"
            );
        }
        assert!(
            node.descendants()
                .any(|node| node.is_text() && node.text() == Some("WIDE")),
            "fixture must retain its authored WIDE label"
        );
    }

    let typed = theme(false, true);
    let diagram = "flowchart LR\nA@{ shape: bpmn-start, label: \"WIDE\" }\n";
    for layout in LAYOUTS {
        let expected = render(&format!("{diagram}style A font-size:32px\n"), None, layout);
        assert_bpmn_start_fixture(expected.svg());
        for class in [
            "label",
            "nodeLabel",
            "bpmn-event",
            "bpmn-event-start",
            "bpmn-event-body",
            "bpmn-annotation-body",
            "text-inner-tspan",
        ] {
            let matching = matches!(
                class,
                "label" | "nodeLabel" | "bpmn-event" | "bpmn-event-start"
            );
            let source = format!("{diagram}classDef {class} font-size:32px\n");
            let rendered = render(&source, Some(&typed), layout);
            assert_bpmn_start_fixture(rendered.svg());
            if matching {
                assert_eq!(
                    label_size(rendered.svg()),
                    label_size(expected.svg()),
                    "{layout}, {class}"
                );
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(
                evidence.applied_count(),
                usize::from(!matching),
                "{layout}, {class}"
            );
            assert_eq!(evidence.not_applicable_count(), usize::from(matching));
            assert_eq!(evidence.theme_residual_count(), 0);

            // Native shape selectors do not reach text/tspan font declarations.
            let native = render_mode(&source, Some(&typed), layout, false);
            assert_bpmn_start_fixture(native.svg());
            let completion = native.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 1, "{layout}, native {class}");
            assert_eq!(evidence.not_applicable_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}
