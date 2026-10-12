#![cfg(all(feature = "native-fonts", feature = "diagram-swimlane"))]

use merman::svg::export::NativeFontOptions;
use merman::{OperationControl, RenderRequest, RenderTarget, Renderer, SvgEnvironment, SvgRequest};

fn request(native: bool) -> SvgRequest {
    let mut request = SvgRequest::default();
    if native {
        let context = NativeFontOptions::default()
            .with_font_file(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../merman-export/tests/fonts/Cousine-Regular.ttf"),
            )
            .load(&OperationControl::new())
            .unwrap();
        request.environment = SvgEnvironment::deterministic()
            .with_native_font_context(context)
            .unwrap();
    }
    request
}

fn assert_title_admission(source: &str, rejected: bool) {
    let native = Renderer::new().render(RenderRequest::new(
        source,
        RenderTarget::Svg(request(true)),
        OperationControl::new(),
    ));
    if rejected {
        let error = native
            .err()
            .expect("rendered inline title typography must fail native admission");
        assert!(
            error
                .to_string()
                .contains("inline HTML text geometry style"),
            "{source}: {error}"
        );
        assert!(error.to_string().contains("font-size"), "{source}: {error}");
        assert!(
            error.to_string().contains("deterministic"),
            "{source}: {error}"
        );
    } else {
        assert!(native.is_ok(), "{source}: {:?}", native.err());
    }
    let compatibility = Renderer::new().render(RenderRequest::new(
        source,
        RenderTarget::Svg(request(false)),
        OperationControl::new(),
    ));
    assert!(compatibility.is_ok(), "{source}: {:?}", compatibility.err());
}

#[test]
fn native_swimlane_title_admission_uses_local_html_labels() {
    for (config, rejected) in [
        (
            r#"{"htmlLabels":false,"flowchart":{"htmlLabels":true}}"#,
            true,
        ),
        (
            r#"{"htmlLabels":true,"flowchart":{"htmlLabels":false}}"#,
            false,
        ),
        (
            r#"{"htmlLabels":false,"flowchart":{"htmlLabels":false}}"#,
            false,
        ),
        (
            r#"{"htmlLabels":true,"flowchart":{"htmlLabels":true}}"#,
            true,
        ),
        (
            r#"{"htmlLabels":false,"flowchart":{"htmlLabels":"true"}}"#,
            true,
        ),
        (
            r#"{"htmlLabels":true,"flowchart":{"htmlLabels":"false"}}"#,
            false,
        ),
        // Config merging ignores null overrides, retaining the default HTML title mode.
        (
            r#"{"htmlLabels":true,"flowchart":{"htmlLabels":null}}"#,
            true,
        ),
        (r#"{"htmlLabels":true,"flowchart":{"htmlLabels":0}}"#, false),
        (r#"{"htmlLabels":false}"#, true),
    ] {
        let source = format!(
            "%%{{init: {config}}}%%\nswimlane-beta LR\nsubgraph Lane[\"Before <span style='font-size:64px'>AVAVAV</span> after\"]\nA[Measured label]-->B\nend\n"
        );
        assert_title_admission(&source, rejected);
    }
}

#[test]
fn native_swimlane_title_admission_follows_effective_markdown_html() {
    for root_html in [false, true] {
        for label in [
            "Literal: `<span style='font-size:64px'>AVAVAV</span>`",
            r"Literal: \<span style='font-size:64px'>AVAVAV\</span>",
        ] {
            for markdown in [false, true] {
                let title = if markdown {
                    format!("`{label}`")
                } else {
                    label.to_owned()
                };
                let source = format!(
                    "%%{{init: {{\"htmlLabels\":{root_html},\"flowchart\":{{\"htmlLabels\":true}}}}}}%%\nswimlane-beta LR\nsubgraph Lane[\"{title}\"]\nA[Measured label]-->B\nend\n"
                );
                assert_title_admission(&source, true);
            }
        }
    }
}

#[test]
fn native_swimlane_nested_titles_use_ordinary_flowchart_html_labels() {
    for (config, rejected) in [
        (
            r#"{"htmlLabels":false,"flowchart":{"htmlLabels":true}}"#,
            false,
        ),
        (
            r#"{"htmlLabels":true,"flowchart":{"htmlLabels":false}}"#,
            true,
        ),
    ] {
        let source = format!(
            "%%{{init: {config}}}%%\nswimlane-beta LR\nsubgraph Lane[Parent lane]\nsubgraph Nested[\"Before <span style='font-size:64px'>AVAVAV</span> after\"]\nA[Measured label]-->B\nend\nend\n"
        );
        assert_title_admission(&source, rejected);
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn native_flowchart_with_swimlane_layout_uses_swimlane_title_admission() {
    for (root_html, local_html, rejected) in [(false, true, true), (true, false, false)] {
        let config = serde_json::json!({
            "layout": "swimlane",
            "htmlLabels": root_html,
            "flowchart": { "htmlLabels": local_html },
        });
        let source = format!(
            "%%{{init: {config}}}%%\nflowchart LR\nsubgraph Lane[\"Before <span style='font-size:64px'>AVAVAV</span> after\"]\nA[Measured label]-->B\nend\n"
        );
        assert_title_admission(&source, rejected);
    }
}
