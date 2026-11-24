#![cfg(feature = "diagram-mindmap")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::OperationControl;
use merman_model::presentation::PresentationDiagram;
use serde_json::{Value, json};

fn parsed(label: &str, config: Value) -> PresentationDiagram {
    MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(
            &format!("mindmap\n  root[\"{label}\"]\n"),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[test]
fn mindmap_native_admission_rejects_unmeasured_html_geometry() {
    for (style, property) in [
        ("letter-spacing:30px", "letter-spacing"),
        ("word-spacing:30px", "word-spacing"),
        ("text-transform:uppercase", "text-transform"),
        ("font:italic 30px Cousine", "font"),
        ("line-height:4", "line-height"),
        ("LETTER-SPACING:30px !important", "letter-spacing"),
        (r"letter\2d spacing:30px", "letter-spacing"),
    ] {
        let parsed = parsed(&format!("<span style='{style}'>AAAA</span>"), json!({}));
        assert_eq!(
            merman_render::__private::unsupported_native_html_style(
                &parsed,
                &OperationControl::new(),
            )
            .unwrap()
            .as_deref(),
            Some(property),
            "{style}"
        );
    }
}

#[test]
fn mindmap_native_admission_reads_property_syntax_independently_of_emission_safety() {
    for (style, expected) in [
        ("letter-spacing:/* comment */30px", Some("letter-spacing")),
        ("word-spacing:/* comment */30px", Some("word-spacing")),
        (
            "text-transform:/* comment */uppercase",
            Some("text-transform"),
        ),
        (
            r"/* leading */LETTER\2d SPACING/* property */:/* value */30px !important;",
            Some("letter-spacing"),
        ),
        ("color:red;line-height:/* value */4;", Some("line-height")),
        ("letter-spacing:/* comment */inherit;", None),
        ("letter-spacing:/* comment */unset !important;", None),
        (
            r"letter-spacing:/* comment */\69nherit /**/ !important;",
            None,
        ),
        ("letter-spacing:/* comment only */;color:red", None),
        ("color:/* letter-spacing:30px; */red;", None),
        ("--probe:letter-spacing:30px;color:red", None),
        ("all:initial", Some("all")),
        ("font-size:64px;all:inherit", Some("all")),
        ("font-size:64px;all:unset", Some("all")),
        ("font-size:64px;font:inherit", Some("font")),
        ("font-size:64px;font:unset", Some("font")),
        (
            "font-size:calc(48px)",
            Some("font-size (unresolved CSS value)"),
        ),
        ("font-size:calc(48px);font-size:24px", None),
        (
            r"font-size:64px;\61ll:/* value */\75nset !important",
            Some("all"),
        ),
    ] {
        let label = format!("<span style='{style}'>AAAA</span>");
        let source = parsed(&label, json!({}));
        assert_eq!(
            merman_render::__private::unsupported_native_html_style(
                &source,
                &OperationControl::new()
            )
            .unwrap()
            .as_deref(),
            expected,
            "{style}"
        );
        if expected.is_some() {
            for config in [
                json!({"htmlLabels": false}),
                json!({"securityLevel": "loose", "dompurifyConfig": {"FORBID_ATTR": ["style"]}}),
            ] {
                assert_eq!(
                    merman_render::__private::unsupported_native_html_style(
                        &parsed(&label, config.clone()),
                        &OperationControl::new(),
                    )
                    .unwrap(),
                    None,
                    "non-HTML or removed declarations must not reject: {style}, {config}"
                );
            }
        }
    }
}

#[test]
fn mindmap_native_admission_keeps_supported_font_longhands_and_literal_text() {
    for (label, config) in [
        (
            "<span style='font-family:Cousine;font-size:2em;font-weight:700;font-style:italic'>AAAA</span>",
            json!({}),
        ),
        (
            "<span style='letter-spacing:inherit'>AAAA</span>",
            json!({}),
        ),
        (
            "<span style='letter-spacing:30px'>AAAA</span>",
            json!({"htmlLabels": false}),
        ),
        (
            "&lt;span style='letter-spacing:30px'&gt;AAAA&lt;/span&gt;",
            json!({}),
        ),
        ("$$x^2$$", json!({})),
    ] {
        assert_eq!(
            merman_render::__private::unsupported_native_html_style(
                &parsed(label, config),
                &OperationControl::new(),
            )
            .unwrap(),
            None,
            "{label}"
        );
    }
}

#[test]
fn mindmap_native_admission_observes_sanitizer_removals_and_rem_ownership() {
    let label = "<span style='letter-spacing:30px'>AAAA</span>";
    let config = json!({
        "securityLevel": "loose",
        "dompurifyConfig": {"FORBID_ATTR": ["style"]}
    });
    assert_eq!(
        merman_render::__private::unsupported_native_html_style(
            &parsed(label, config),
            &OperationControl::new(),
        )
        .unwrap(),
        None,
    );
    for (style, rejected) in [
        ("font-size:2rem", true),
        ("font-size:2rem;font-size:24px", false),
        ("font-size:2rem !important;font-size:24px", true),
    ] {
        let source = parsed(&format!("<span style='{style}'>AAAA</span>"), json!({}));
        assert_eq!(
            merman_render::__private::unsupported_native_html_style(
                &source,
                &OperationControl::new(),
            )
            .unwrap()
            .is_some(),
            rejected,
            "{style}"
        );
    }
}

#[test]
fn mindmap_native_admission_preserves_cancellation_during_label_preparation() {
    let label = format!(
        "{}<span style='letter-spacing:30px'>AAAA</span>",
        "word ".repeat(8192)
    );
    let source = parsed(&label, json!({}));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(8);
    let error = merman_render::__private::unsupported_native_html_style(&source, &control)
        .expect_err("native admission must remain cancellable within a large label");
    assert_eq!(error.reason, merman_model::CancelReason::Requested);
}
