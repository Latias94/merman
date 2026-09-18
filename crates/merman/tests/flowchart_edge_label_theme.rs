#![cfg(feature = "svg")]

use merman::svg::{DiagramTheme, SvgPipeline, ThemePortabilityRequirement};
#[cfg(feature = "embedded-fonts")]
use merman::svg::{
    DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec,
    FontEmbeddingRequirement, FontStack, Specified, TextStylePatch, ThemeAssets, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
#[cfg(feature = "embedded-fonts")]
use merman::{DiagramFamilyId, Engine, MermaidConfig};
use merman::{
    OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgOutput, SvgRequest,
    ThemeEvidenceStatus,
};

#[cfg(feature = "embedded-fonts")]
fn edge_label_typography_styles_with_font_stack(
    family: DiagramFamilyId,
    font_family: &str,
) -> ThemeRuleSet {
    ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(
                ThemeTarget::EdgeLabel,
                ThemeStylePatch {
                    typography: TextStylePatch {
                        font_stack: Specified::Value(
                            FontStack::single(font_family).expect("valid fixture font stack"),
                        ),
                        ..TextStylePatch::default()
                    },
                    ..ThemeStylePatch::default()
                },
            )
            .for_family(family),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::EdgeLabel,
                ThemeStylePatch {
                    typography: TextStylePatch {
                        font_size_px: Specified::Value(26.0),
                        ..TextStylePatch::default()
                    },
                    ..ThemeStylePatch::default()
                },
            )
            .with_variant(ThemeVariant::Default)
            .for_family(family),
        )
}

#[cfg(feature = "embedded-fonts")]
fn edge_label_typography_theme(family: DiagramFamilyId) -> DiagramTheme {
    edge_label_typography_theme_with_font_stack(family, "Excalifont")
}

#[cfg(feature = "embedded-fonts")]
fn edge_label_typography_theme_with_font_stack(
    family: DiagramFamilyId,
    edge_font_family: &str,
) -> DiagramTheme {
    let latin = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let cjk = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
    ));
    let styles = edge_label_typography_styles_with_font_stack(family, edge_font_family).with_rule(
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch {
                typography: TextStylePatch {
                    font_stack: Specified::Value(
                        FontStack::single("Excalifont").expect("valid fixture font stack"),
                    ),
                    ..TextStylePatch::default()
                },
                ..ThemeStylePatch::default()
            },
        )
        .for_family(family),
    );
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(styles).with_assets(
                ThemeAssets::default().with_font_catalog(
                    FontCatalogSpec::new([
                        FontAssetSpec::new("excalifont", latin),
                        FontAssetSpec::new("xiaolai", cjk),
                    ])
                    .with_embedding_requirement(FontEmbeddingRequirement::FullFont),
                ),
            ),
        )
        .expect("compile EdgeLabel typography theme")
}

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

#[cfg(feature = "embedded-fonts")]
fn edge_label_text_styles(svg: &str) -> Vec<String> {
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
        .filter_map(|node| node.attribute("style").map(str::to_string))
        .collect()
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

#[cfg(feature = "embedded-fonts")]
fn assert_typed_edge_label_style(output: &SvgOutput) {
    let styles = edge_label_text_styles(output.svg());
    assert!(
        styles
            .iter()
            .any(|style| style.contains("font-family:\"Excalifont\"")),
        "styles={styles:?}, svg={}",
        output.svg()
    );
    assert!(
        styles.iter().any(|style| style.contains("font-size:26px")),
        "styles={styles:?}, svg={}",
        output.svg()
    );
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn typed_edge_label_typography_is_verified_for_flowchart_and_swimlane() {
    for (family, source) in [
        (
            DiagramFamilyId::FLOWCHART,
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: start }
B@{ shape: stop }
A -->|typed edge label| B
"#,
        ),
        (
            DiagramFamilyId::SWIMLANE,
            r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: start }
B@{ shape: stop }
A -->|typed edge label| B
"#,
        ),
    ] {
        let output = render(source, Some(edge_label_typography_theme(family)), true)
            .expect("prepared EdgeLabel typography should satisfy strict portability");

        assert_typed_edge_label_style(&output);
        assert_eq!(
            output.evidence().theme_evidence().status(),
            ThemeEvidenceStatus::Verified
        );
    }
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn source_and_config_shadow_edge_label_typography_per_facet() {
    let source_font_size = render(
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: start }
B@{ shape: stop }
A -->|source font size| B
linkStyle 0 font-size:22px
"#,
        Some(edge_label_typography_theme(DiagramFamilyId::FLOWCHART)),
        true,
    )
    .expect("source font size should shadow only typed EdgeLabel font size");
    let source_styles = edge_label_text_styles(source_font_size.svg());
    assert!(
        source_styles
            .iter()
            .any(|style| style.contains("font-family:\"Excalifont\"")),
        "{source_styles:?}"
    );
    assert!(
        source_styles
            .iter()
            .any(|style| style.contains("font-size:22px")),
        "{source_styles:?}"
    );
    assert!(
        source_styles
            .iter()
            .all(|style| !style.contains("font-size:26px")),
        "{source_styles:?}"
    );
    assert_eq!(
        source_font_size.evidence().theme_evidence().status(),
        ThemeEvidenceStatus::Verified
    );

    let configured_font_stack = render_with_renderer(
        Renderer::new().with_engine(Engine::new().with_site_config(MermaidConfig::from_value(
            serde_json::json!({
                "htmlLabels": false,
                "flowchart": {"htmlLabels": false},
                "themeVariables": {"fontFamily": "Excalifont"}
            }),
        ))),
        r#"flowchart LR
A@{ shape: start }
B@{ shape: stop }
A -->|typed edge label| B
"#,
        Some(edge_label_typography_theme_with_font_stack(
            DiagramFamilyId::FLOWCHART,
            "Xiaolai SC",
        )),
        true,
    )
    .expect("Mermaid font family should shadow only typed EdgeLabel font stack");
    let configured_styles = edge_label_text_styles(configured_font_stack.svg());
    assert!(
        configured_styles
            .iter()
            .any(|style| style.contains("font-family:\"Excalifont\"")),
        "{configured_styles:?}"
    );
    assert!(
        configured_styles
            .iter()
            .any(|style| style.contains("font-size:26px")),
        "{configured_styles:?}"
    );
    assert!(
        configured_styles
            .iter()
            .all(|style| !style.contains("font-family:\"Xiaolai SC\"")),
        "{configured_styles:?}"
    );
    assert_eq!(
        configured_font_stack.evidence().theme_evidence().status(),
        ThemeEvidenceStatus::Verified
    );
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn edge_label_typography_is_satisfied_without_edge_labels() {
    let output = render(
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: start }
B@{ shape: stop }
A --> B
"#,
        Some(edge_label_typography_theme(DiagramFamilyId::FLOWCHART)),
        true,
    )
    .expect("an unlabeled edge should not require EdgeLabel typography");

    assert!(output.evidence().theme_evidence().is_satisfied());
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn html_edge_labels_fail_closed_before_a_prepared_token_exists() {
    let error = render(
        "flowchart LR\nA@{ shape: start }\nB@{ shape: stop }\nA -->|HTML edge label| B\n",
        Some(edge_label_typography_theme(DiagramFamilyId::FLOWCHART)),
        true,
    )
    .expect_err("HTML EdgeLabel typography must not claim a prepared writer receipt");

    assert!(
        error.contains("not supported by prepared text layout"),
        "{error}"
    );
}

#[test]
#[cfg(feature = "embedded-fonts")]
fn one_markdown_edge_label_cannot_be_hidden_by_a_verified_plain_label() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: start }
B@{ shape: start }
C@{ shape: stop }
A -->|prepared plain label| B
B -->|"`**unprepared markdown label**`"| C
"#;
    let theme = edge_label_typography_theme(DiagramFamilyId::FLOWCHART);
    let best_effort = render(source, Some(theme.clone()), false)
        .expect("best-effort output should retain the unverified Markdown residual");

    assert_eq!(
        best_effort.evidence().theme_evidence().status(),
        ThemeEvidenceStatus::Residual
    );
    assert!(
        render(source, Some(theme), true).is_err(),
        "strict output must reject one applicable EdgeLabel without a prepared receipt"
    );
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
