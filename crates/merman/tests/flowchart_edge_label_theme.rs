#![cfg(feature = "svg")]

use merman::svg::{DiagramTheme, SvgPipeline, ThemePortabilityRequirement};
use merman::{
    OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgOutput, SvgRequest,
    ThemeEvidenceStatus,
};

fn svg_request(strict: bool) -> SvgRequest {
    let environment = if strict {
        SvgEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
    } else {
        SvgEnvironment::deterministic()
    };
    SvgRequest {
        environment,
        pipeline: strict.then(SvgPipeline::resvg_safe),
        ..SvgRequest::default()
    }
}

fn render(source: &str, theme: Option<DiagramTheme>, strict: bool) -> Result<SvgOutput, String> {
    render_with_renderer(Renderer::new(), source, theme, strict)
}

fn render_with_renderer(
    renderer: Renderer,
    source: &str,
    theme: Option<DiagramTheme>,
    strict: bool,
) -> Result<SvgOutput, String> {
    let request = RenderRequest::svg(source, OperationControl::new(), svg_request(strict));
    let request = match theme {
        Some(theme) => request.with_theme(theme),
        None => request,
    };
    match renderer
        .render(request)
        .map_err(|error| error.to_string())?
    {
        RenderOutput::Svg(Some(output)) => Ok(output),
        RenderOutput::Svg(None) => Err("source did not contain a diagram".to_string()),
        other => Err(format!("unexpected render output: {other:?}")),
    }
}

fn edge_label_text_content(svg: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG XML");
    document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node.ancestors().any(|ancestor| {
                    ancestor.has_tag_name("g")
                        && ancestor.attribute("class").is_some_and(|classes| {
                            classes
                                .split_ascii_whitespace()
                                .any(|class| class == "edgeLabel")
                        })
                })
        })
        .map(|node| {
            node.descendants()
                .filter(|part| part.is_text())
                .filter_map(|part| part.text())
                .collect()
        })
        .collect()
}

#[test]
fn unthemed_edge_labels_keep_the_legacy_svg_path() {
    let output = render(
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A -->|legacy edge label| B
"#,
        None,
        false,
    )
    .expect("unthemed Flowchart should render");

    assert_eq!(
        output.evidence().theme_evidence().status(),
        ThemeEvidenceStatus::NotApplicable
    );
    assert!(
        !output.svg().contains("merman-prepared-"),
        "{}",
        output.svg()
    );
    assert!(
        edge_label_text_content(output.svg())
            .iter()
            .any(|text| text == "legacy edge label"),
        "legacy path should retain the authored edge label: {}",
        output.svg(),
    );
}
