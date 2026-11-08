#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use merman_model::{CancelReason, OperationControl};
use merman_render::__private::unsupported_native_html_style;
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

fn parsed(label: &str, config: Value) -> PresentationDiagram {
    let source = format!("---\nconfig: {config}\n---\nerDiagram\nA ||--|| B : \"{label}\"\n");
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[test]
fn er_relationship_native_admission_reads_retained_css_without_expanding_emission_safety() {
    for (style, expected) in [
        ("letter-spacing:20px", Some("letter-spacing")),
        ("letter-spacing:/* value */20px", Some("letter-spacing")),
        (
            r"/* leading */LETTER\2d SPACING/* property */:/* value */20px !important;",
            Some("letter-spacing"),
        ),
        ("color:red;line-height:/* value */4", Some("line-height")),
        ("letter-spacing:/* value */inherit", None),
        (r"letter-spacing:/* value */\75nset !important", None),
        ("letter-spacing:/* comment only */;color:red", None),
        ("color:/* letter-spacing:20px; */red", None),
        ("--probe:letter-spacing:20px;color:red", None),
        ("all:initial", Some("all")),
        ("all:/* value */initial !important", Some("all")),
        ("all:inherit", Some("all")),
        ("all:unset", Some("all")),
        ("font-size:64px;all:inherit", Some("all")),
        (
            r"font-size:64px;\61ll:/* value */\75nset !important",
            Some("all"),
        ),
        (
            "font-family:Cousine;font-size:2em;font-weight:700;font-style:italic",
            None,
        ),
        (
            "font-size:2rem",
            Some("font-size (rem requires a document-root font size)"),
        ),
        ("font-size:2rem;font-size:24px", None),
    ] {
        let label = format!("<span style='{style}'>AAAA</span>");
        let source = parsed(&label, json!({"layout": "swimlane", "htmlLabels": true}));
        assert_eq!(
            unsupported_native_html_style(&source, &OperationControl::new())
                .unwrap()
                .as_deref(),
            expected,
            "{style}"
        );
    }
}

#[test]
fn er_relationship_native_admission_observes_svg_and_sanitizer_removals() {
    for style in [
        "letter-spacing:20px",
        "all:initial",
        "font-size:64px;all:inherit",
        "font-size:64px;all:unset",
    ] {
        for (html, remove_style, rejected) in [
            (true, false, true),
            (false, false, false),
            (true, true, false),
        ] {
            let mut config = json!({"layout": "swimlane", "htmlLabels": html});
            if remove_style {
                config["dompurifyConfig"] = json!({"FORBID_ATTR": ["style"]});
            }
            let label = format!("<span style='{style}'>AAAA</span>");
            let source = parsed(&label, config);
            assert_eq!(
                unsupported_native_html_style(&source, &OperationControl::new())
                    .unwrap()
                    .is_some(),
                rejected,
            );
            let artifact = family::prepare(
                &source,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .unwrap();
            let svg = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap()
                .svg()
                .to_owned();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let relationship = document
                .descendants()
                .find(|node| {
                    node.attribute("id")
                        .is_some_and(|id| id.starts_with("edge-label-"))
                })
                .unwrap();
            assert_eq!(
                relationship
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                html,
            );
            assert_eq!(
                relationship.descendants().any(|node| {
                    node.attribute("style")
                        .is_some_and(|retained| retained.contains(style))
                }),
                rejected,
                "admission must inspect the declarations actually retained by the writer"
            );
        }
    }
}

#[test]
fn er_dagre_native_admission_rejects_malformed_retained_html() {
    let source = parsed(
        "<b>x</i><span style='letter-spacing:20px'>AAAA</span>",
        json!({"htmlLabels": true}),
    );
    assert_eq!(
        unsupported_native_html_style(&source, &OperationControl::new())
            .unwrap()
            .as_deref(),
        Some("HTML label normalization"),
    );
    let control = OperationControl::new();
    control.cancel();
    assert_eq!(
        unsupported_native_html_style(&source, &control)
            .unwrap_err()
            .reason,
        CancelReason::Requested,
    );
}

#[test]
fn er_native_html_admission_ignores_entities_hidden_by_subgraph_names() {
    for provider in ["swimlane", "dagre"] {
        for (entity_name, rejected) in [("G", false), ("H", true)] {
            let source = format!(
                "---\nconfig: {}\n---\nerDiagram\n{entity_name}[\"<span style='letter-spacing:20px'>AAAA</span>\"]\nsubgraph G[Visible]\nA\nend\n",
                json!({"layout": provider, "htmlLabels": true}),
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let RenderSemanticModel::Er(model) = parsed.model() else {
                panic!("expected ER model");
            };
            let entity = &model.entities[entity_name];
            assert!(
                entity
                    .label_source_for_render()
                    .contains("letter-spacing:20px")
            );
            assert_eq!(
                unsupported_native_html_style(&parsed, &OperationControl::new())
                    .unwrap()
                    .as_deref(),
                rejected.then_some("letter-spacing"),
                "{provider}: {entity_name}",
            );
            let artifact = family::prepare(
                &parsed,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .unwrap();
            let svg = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap()
                .svg()
                .to_owned();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let dom_id = format!("merman-{}", entity.id);
            assert_eq!(
                document
                    .descendants()
                    .any(|node| node.attribute("id") == Some(dom_id.as_str())),
                rejected,
            );
            assert_eq!(svg.contains("letter-spacing:20px"), rejected);
        }
    }
}

#[test]
fn er_relationship_native_admission_preserves_cancellation_during_html_recovery() {
    let label = format!(
        "{}<span style='letter-spacing:20px'>AAAA</span>",
        "<span>word </span>".repeat(1024)
    );
    let source = parsed(&label, json!({"layout": "swimlane", "htmlLabels": true}));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(64);
    let error = unsupported_native_html_style(&source, &control).unwrap_err();
    assert_eq!(error.reason, CancelReason::Requested);
    assert_eq!(control.checkpoint(), Err(error));
}

#[test]
fn er_native_html_admission_follows_each_groups_actual_writer() {
    for (root, flowchart) in [(false, true), (true, false)] {
        for (styled_group, html) in [("Outer", flowchart), ("Inner", root)] {
            let title = |id: &str| {
                if id == styled_group {
                    "<span style='letter-spacing:20px'>AAAA</span>"
                } else {
                    "Plain title"
                }
            };
            let source = format!(
                "---\nconfig: {}\n---\nerDiagram\nsubgraph Outer[\"{}\"]\nsubgraph Inner[\"{}\"]\nA\nend\nend\n",
                json!({"layout": "swimlane", "htmlLabels": root,
                    "flowchart": {"htmlLabels": flowchart}}),
                title("Outer"),
                title("Inner"),
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let RenderSemanticModel::Er(model) = parsed.model() else {
                panic!("expected ER model");
            };
            assert!(
                model
                    .subgraphs
                    .iter()
                    .all(|group| group.label_type == "markdown")
            );
            assert_eq!(
                unsupported_native_html_style(&parsed, &OperationControl::new())
                    .unwrap()
                    .as_deref(),
                html.then_some("letter-spacing"),
                "{styled_group}: root={root}, flowchart={flowchart}"
            );
        }
    }
}
