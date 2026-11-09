#![cfg(feature = "diagram-requirement")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_model::{CancelReason, OperationControl, OperationPhase};
use merman_render::__private::{unsupported_native_html_style, unsupported_native_text_style};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::{Value, json};

fn parsed(body: &str, config: Value) -> PresentationDiagram {
    MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(body, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

fn label_source(style: &str) -> String {
    format!(
        "requirementDiagram\nrequirement r {{\ntext: \"<span style='{style}'>WWWWWWWW</span>\"\n}}\n"
    )
}

#[test]
fn requirement_native_html_checks_retained_declarations_and_accepts_measured_longhands() {
    for (style, expected) in [
        ("font:italic 48px Cousine", Some("font")),
        ("font-size:64px;font:inherit", Some("font")),
        ("font-size:64px;font:unset", Some("font")),
        ("letter-spacing:12px", Some("letter-spacing")),
        (
            r"/* lead */LETTER\2d SPACING:/* value */12px",
            Some("letter-spacing"),
        ),
        ("all:inherit", Some("all")),
        ("font-size:64px;all:unset", Some("all")),
        ("font-style:italic;font-size:48px;font-family:Cousine", None),
        ("font-size:24pt;font-weight:700", None),
        ("font-size:2em", None),
        ("font-size:0px", None),
        ("letter-spacing:inherit", None),
        ("color:/* letter-spacing:12px */red", None),
        (
            "font-size:2rem",
            Some("font-size (rem requires a document-root font size)"),
        ),
        ("font-size:2rem;font-size:24pt", None),
        (
            "font-size:calc(48px)",
            Some("font-size (unresolved CSS value)"),
        ),
        ("font-size:calc(48px);font-size:24px", None),
        (
            "font-size:calc(48px) !important;font-size:24px",
            Some("font-size (unresolved CSS value)"),
        ),
        (
            "font-size:calc(48px) !important;font-size:inherit !important",
            None,
        ),
    ] {
        let diagram = parsed(&label_source(style), json!({"htmlLabels": true}));
        assert_eq!(
            unsupported_native_html_style(&diagram, &OperationControl::new())
                .unwrap()
                .as_deref(),
            expected,
            "{style}",
        );
    }
}

#[test]
fn requirement_native_html_uses_node_mode_and_sanitized_projection() {
    for style in [
        "font:italic 48px Cousine",
        "font-size:64px;font:inherit",
        "font-size:64px;font:unset",
        "font-size:calc(48px)",
        "letter-spacing:12px",
        "all:initial",
    ] {
        for (config, html, retained) in [
            (json!({"htmlLabels": true}), true, true),
            (
                json!({"htmlLabels": false, "flowchart": {"htmlLabels": true}}),
                false,
                false,
            ),
            (json!({"flowchart": {"htmlLabels": false}}), true, true),
            (
                json!({"htmlLabels": true, "dompurifyConfig": {"FORBID_ATTR": ["style"]}}),
                true,
                false,
            ),
        ] {
            let diagram = parsed(&label_source(style), config);
            assert_eq!(
                unsupported_native_html_style(&diagram, &OperationControl::new())
                    .unwrap()
                    .is_some(),
                retained,
                "{style}",
            );
            let artifact = family::prepare(
                &diagram,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .unwrap();
            let rendered = artifact
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some("requirement-native".into()),
                        ..Default::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|node| node.attribute("id") == Some("requirement-native-r"))
                .unwrap();
            assert_eq!(
                node.descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                html
            );
            assert_eq!(
                node.descendants().any(|node| {
                    node.attribute("style")
                        .is_some_and(|value| value.contains(style))
                }),
                retained
            );
        }
    }
}

#[test]
fn requirement_native_html_inspects_element_rows_and_ignores_escaped_markup() {
    let element = "requirementDiagram\nelement e {\ntype: \"<span style='font:italic 48px Cousine'>Component</span>\"\n}\n";
    assert_eq!(
        unsupported_native_html_style(&parsed(element, json!({})), &OperationControl::new())
            .unwrap()
            .as_deref(),
        Some("font"),
    );
    let escaped = "requirementDiagram\nrequirement r {\ntext: \"`<span style='font:italic 48px Cousine'>literal</span>`\"\n}\n";
    assert_eq!(
        unsupported_native_html_style(&parsed(escaped, json!({})), &OperationControl::new())
            .unwrap(),
        None,
    );
}

#[test]
fn requirement_native_html_skips_element_rows_replaced_by_a_same_named_requirement() {
    for (requirement_text, element_type, rejected) in [
        (
            "Visible",
            "<span style='font:italic 48px Cousine'>Hidden</span>",
            false,
        ),
        (
            "<span style='font:italic 48px Cousine'>Visible</span>",
            "Hidden",
            true,
        ),
    ] {
        let source = format!(
            "requirementDiagram\nrequirement shared {{\ntext: \"{requirement_text}\"\n}}\nelement shared {{\ntype: \"{element_type}\"\n}}\n",
        );
        let diagram = parsed(&source, json!({"htmlLabels": true}));
        assert_eq!(
            unsupported_native_html_style(&diagram, &OperationControl::new())
                .unwrap()
                .is_some(),
            rejected,
        );
    }
}

#[test]
fn requirement_native_node_size_boundary_accepts_absolute_units_and_final_inheritance() {
    for (declarations, rejected) in [
        ("font-size:24pt", false),
        (r"font-size:24p\74", false),
        ("font-size:2PC", false),
        ("font-size:0", false),
        ("font-size:0.5px", false),
        ("font-size:2em", true),
        ("font-size:125%", true),
        ("font-size:calc(24pt)", true),
        ("font-size:var(--size)", true),
        ("font-size:2rem !important,font-size:24pt", false),
        ("font-size:2em,font-size:inherit", false),
        ("font-size:24pt,font-size:2em", true),
    ] {
        let source = format!(
            "requirementDiagram\nrequirement r {{\ntext: WWWW\n}}\nstyle r {declarations}\n"
        );
        let diagram = parsed(&source, json!({}));
        assert_eq!(
            unsupported_native_text_style(&diagram, &OperationControl::new())
                .unwrap()
                .is_some(),
            rejected,
            "{declarations}",
        );
    }
}

#[test]
fn requirement_native_node_styles_skip_a_shadowed_element() {
    for (hidden_style, visible_style, rejected) in [
        ("font-size:2em", "", false),
        ("font:italic 48px Cousine", "", false),
        ("font-size:24pt", "style shared font-size:2em\n", true),
    ] {
        let source = format!(
            "requirementDiagram\nelement shared {{\ntype: Hidden\n}}\nstyle shared {hidden_style}\nrequirement shared {{\ntext: Visible\n}}\n{visible_style}",
        );
        let diagram = parsed(&source, json!({}));
        assert_eq!(
            unsupported_native_text_style(&diagram, &OperationControl::new())
                .unwrap()
                .is_some(),
            rejected,
            "{source}",
        );
        if !rejected {
            let artifact = family::prepare(
                &diagram,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            assert!(!rendered.svg().contains(hidden_style), "{hidden_style}");
        }
    }
}

#[test]
fn requirement_native_html_cancels_during_retained_label_normalization() {
    let source = format!(
        "requirementDiagram\nrequirement r {{\ntext: \"{}<span style='font:italic 48px Cousine'>WWWW</span>\"\n}}\n",
        "<span>word </span>".repeat(1024),
    );
    let diagram = parsed(&source, json!({"htmlLabels": true}));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(128);
    let error = unsupported_native_html_style(&diagram, &control).unwrap_err();
    assert_eq!(error.reason, CancelReason::Requested);
    assert_eq!(error.phase, OperationPhase::Semantic);
}
