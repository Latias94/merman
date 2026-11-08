use merman_core::{MermaidEngine, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::{Value, json};

const LAYOUTS: &[&str] = &[
    "dagre",
    #[cfg(feature = "layout-elk")]
    "elk",
    "swimlane",
];

fn render_cluster(config: Value, source_style: &str, paint: CanvasPaint) -> (String, usize, usize) {
    let (svg, applied, not_applicable, _) = render_cluster_with_spec(
        config,
        source_style,
        DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(paint),
                )
                .with_variant(ThemeVariant::Default),
            ),
        ),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render portable ER cluster");
    (svg, applied, not_applicable)
}

fn render_cluster_with_spec(
    config: Value,
    source_style: &str,
    spec: DiagramThemeSpec,
    requirement: ThemePortabilityRequirement,
) -> Result<(String, usize, usize, usize), merman_render::Error> {
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .expect("compile ER cluster text theme");
    let source = format!(
        "---\nconfig: {config}\n---\nerDiagram\nsubgraph Orders [\"**Order** Domain\"]\n CUSTOMER\nend\nstyle CUSTOMER color:#556677\n{source_style}\n"
    );
    let parsed = common::install_parse_compatibility(
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(json!({"secure": []})),
        ),
    )
    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
    .expect("parse ER cluster")
    .expect("detect ER cluster")
    .into_presentation();
    assert_eq!(
        parsed
            .metadata()
            .effective_config
            .as_value()
            .pointer("/themeVariables/titleColor"),
        config.pointer("/themeVariables/titleColor"),
        "the fixture must retain its explicit titleColor owner or fallback"
    );
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(&theme)
        .expect("begin ER cluster session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare ER cluster")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    let svg = rendered.svg().to_owned();
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    if matches!(requirement, ThemePortabilityRequirement::RequirePortable) {
        assert_eq!(evidence.theme_residual_count(), 0);
    }
    Ok((
        svg,
        evidence.applied_count(),
        evidence.not_applicable_count(),
        evidence.theme_residual_count(),
    ))
}

#[test]
fn dagre_duplicate_subgraphs_keep_first_box_style_and_class_without_svg_source_text_color() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile baseline ER theme");
    let source = r##"---
config:
  layout: dagre
  htmlLabels: false
---
erDiagram
classDef first fill:#112233,stroke:#445566,color:#778899
classDef last fill:#aabbcc,stroke:#ddeeff,color:#abcdef
subgraph Shared ["First title"]
 FIRST
end
class Shared first
style Shared stroke-width:2px
subgraph Shared ["Last title"]
 LAST
end
class Shared last
style Shared stroke-width:7px
"##;
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse duplicate ER clusters")
        .expect("detect ER diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin ER session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Dagre duplicate clusters")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render duplicate ER clusters");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid SVG");
    let clusters: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("cluster first"))
        .collect();
    assert!(
        !clusters.is_empty(),
        "the writer must select the first subgraph class"
    );
    for cluster in clusters {
        let rect = cluster
            .children()
            .find(|node| node.has_tag_name("rect"))
            .expect("cluster rectangle");
        let style = rect.attribute("style").expect("prepared source style");
        assert!(style.contains("fill:#112233 !important"), "{style}");
        assert!(style.contains("stroke:#445566 !important"), "{style}");
        assert!(style.contains("stroke-width:2px !important"), "{style}");
        assert!(!style.contains("#aabbcc"), "{style}");
        let label = cluster
            .descendants()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .expect("cluster label");
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .expect("SVG title");
        // The shared upstream cluster painter applies class color only to HTML spans.
        assert_eq!(label.attribute("style"), Some(""));
        assert_eq!(text.attribute("style"), Some(""));
        let title: String = text
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect();
        assert!(title.contains("First"), "{title}");
        assert!(!title.contains("Last"), "{title}");
    }
    assert!(
        !document
            .descendants()
            .any(|node| node.attribute("class") == Some("cluster last"))
    );
}

#[test]
fn svg_cluster_uses_typed_text_fill_when_title_color_selects_the_text_fallback() {
    for look in ["classic", "neo", "handDrawn"] {
        for (paint, color) in [
            (
                CanvasPaint::solid("#123abc").expect("solid text paint"),
                "#123abc",
            ),
            (CanvasPaint::Transparent, "transparent"),
        ] {
            let (svg, applied, not_applicable) = render_cluster(
                json!({"htmlLabels": false, "look": look, "themeVariables": {"titleColor": false}}),
                "",
                paint,
            );
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            let text = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("text")
                        && node
                            .ancestors()
                            .any(|ancestor| ancestor.attribute("class") == Some("cluster-label"))
                })
                .expect("visible SVG cluster text");
            assert_eq!(
                text.attribute("style"),
                Some(format!("color:{color};fill:{color}").as_str()),
                "{look}"
            );
            assert!(
                text.descendants()
                    .any(|node| node.text().is_some_and(|text| text.contains("Order")))
            );
            assert!(text.descendants().any(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("font-weight") == Some("bold")
                    && node.text() == Some("Order")
            }));
            assert_eq!((applied, not_applicable), (1, 0));
        }
    }
}

#[test]
fn cluster_svg_title_uses_config_color_or_typed_fallback_despite_ignored_source_color() {
    for (config, source_style, expected_style, expected_counts) in [
        (
            json!({"htmlLabels": false, "themeVariables": {"titleColor": "#c0ffee"}}),
            "",
            Some(""),
            (0, 1),
        ),
        (
            json!({"htmlLabels": false, "themeVariables": {"titleColor": false, "textColor": "#c0ffee"}}),
            "",
            Some(""),
            (0, 1),
        ),
        (
            json!({"htmlLabels": false, "themeVariables": {"titleColor": false}}),
            "style Orders color:#c0ffee",
            Some("color:#123abc;fill:#123abc"),
            (1, 0),
        ),
        (
            json!({"htmlLabels": true, "themeVariables": {"titleColor": false}}),
            "",
            None,
            (0, 1),
        ),
    ] {
        let (svg, applied, not_applicable) =
            render_cluster(config, source_style, CanvasPaint::solid("#123abc").unwrap());
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let cluster = document
            .descendants()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .expect("cluster label");
        let text = cluster.descendants().find(|node| node.has_tag_name("text"));
        assert_eq!(
            text.and_then(|text| text.attribute("style")),
            expected_style
        );
        assert_eq!((applied, not_applicable), expected_counts);
    }
}

#[test]
fn cluster_source_label_styles_stay_on_html_spans_and_do_not_leak_to_svg_or_member_labels() {
    let cases = [
        (
            "style Orders color:#ff0000,color:#0000ff",
            "color:#0000ff !important",
        ),
        (
            "style Orders color:#0000ff,color:#ff0000",
            "color:#ff0000 !important",
        ),
        ("style Orders color:#c0ffee", "color:#c0ffee !important"),
        (
            "style Orders font-family:SourceFace",
            "font-family:SourceFace !important",
        ),
        ("style Orders font-size:30px", "font-size:30px !important"),
        ("style Orders font-weight:900", "font-weight:900 !important"),
        ("", ""),
    ];
    for layout in LAYOUTS {
        for html_labels in [false, true] {
            for (source_style, html_style) in cases {
                let (svg, _, _, _) = render_cluster_with_spec(
                    json!({
                        "layout": layout,
                        "look": "classic",
                        "htmlLabels": html_labels,
                        "flowchart": {"htmlLabels": html_labels},
                        "themeVariables": {"titleColor": "#112233"}
                    }),
                    source_style,
                    DiagramThemeSpec::new(),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .expect("render ER subgraph source-label-style control");
                let document = roxmltree::Document::parse(&svg).expect("valid ER SVG");
                let cluster = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("merman-Orders"))
                    .expect("authored Orders cluster");
                let label = cluster
                    .descendants()
                    .find(|node| {
                        node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_whitespace()
                                .any(|class| class == "cluster-label")
                        })
                    })
                    .expect("cluster title label");
                let context = format!("{layout}, htmlLabels={html_labels}, {source_style:?}");
                assert_eq!(
                    label
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    html_labels,
                    "{context}"
                );
                if html_labels {
                    let span = label
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("span")
                                && node.attribute("class") == Some("nodeLabel")
                        })
                        .expect("HTML cluster title span");
                    assert_eq!(
                        span.attribute("style").unwrap_or(""),
                        html_style,
                        "{context}"
                    );
                } else {
                    let text = label
                        .descendants()
                        .find(|node| node.has_tag_name("text"))
                        .expect("SVG cluster title text");
                    assert_eq!(label.attribute("style").unwrap_or(""), "", "{context}");
                    assert_eq!(text.attribute("style").unwrap_or(""), "", "{context}");
                    for terminal in label.descendants().filter(|node| node.is_element()) {
                        assert_eq!(terminal.attribute("font-family"), None, "{context}");
                        assert_eq!(terminal.attribute("font-size"), None, "{context}");
                        let style = terminal.attribute("style").unwrap_or("");
                        assert!(!style.contains("font-family"), "{context}: {style}");
                        assert!(!style.contains("font-size"), "{context}: {style}");
                        assert!(!style.contains("font-weight"), "{context}: {style}");
                    }
                    assert!(svg.contains("fill:#112233"), "{context}: title theme role");
                }
                let entity = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node
                                .attribute("id")
                                .is_some_and(|id| id.starts_with("merman-entity-CUSTOMER-"))
                    })
                    .expect("independently styled member entity");
                let entity_label = entity
                    .descendants()
                    .find(|node| node.attribute("class") == Some("label"))
                    .expect("member entity label");
                let entity_style = if html_labels {
                    "color:#556677 !important"
                } else {
                    "fill:#556677 !important"
                };
                assert_eq!(
                    entity_label.attribute("style"),
                    Some(entity_style),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn ignored_svg_subgraph_source_typography_keeps_typed_fonts_portable() {
    for layout in LAYOUTS {
        for source_style in [
            "",
            "style Orders font-family:SourceFace",
            "style Orders font-size:30px",
            "style Orders font-family:SourceFace,font-size:30px",
        ] {
            let spec = DiagramThemeSpec::new().with_typography(
                merman_render::diagram_theme::TypographySpec::default().with_family_style(
                    merman_model::family::DiagramFamilyId::ER,
                    ThemeTextStyle::default()
                        .with_font_stack(FontStack::single("ErTypedFace").expect("font stack"))
                        .with_font_size_px(24.0)
                        .expect("font size"),
                ),
            );
            let (svg, applied, not_applicable, residuals) = render_cluster_with_spec(
                json!({
                    "layout": layout,
                    "htmlLabels": false,
                    "flowchart": {"htmlLabels": false},
                    "themeVariables": {"titleColor": false}
                }),
                source_style,
                spec,
                ThemePortabilityRequirement::RequirePortable,
            )
            .expect("ignored SVG subgraph source fonts must not block portable typed fonts");
            let document = roxmltree::Document::parse(&svg).expect("valid typed ER SVG");
            let cluster = document
                .descendants()
                .find(|node| node.attribute("id") == Some("merman-Orders"))
                .expect("authored Orders cluster");
            let label = cluster
                .descendants()
                .find(|node| {
                    node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "cluster-label")
                    })
                })
                .expect("SVG cluster title label");
            let text = label
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .expect("SVG cluster title text");
            assert_eq!(label.attribute("style").unwrap_or(""), "");
            assert_eq!(text.attribute("style").unwrap_or(""), "");
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .expect("typed ER stylesheet");
            for selector in ["#merman{", "#merman svg{"] {
                let declarations = css
                    .split_once(selector)
                    .and_then(|(_, rule)| rule.split_once('}'))
                    .map(|(declarations, _)| declarations)
                    .expect("root font inheritance rule");
                assert!(
                    declarations.contains("font-family:ErTypedFace;"),
                    "{layout}: {source_style}: {declarations}"
                );
                assert!(
                    declarations.contains("font-size:24px;"),
                    "{layout}: {source_style}: {declarations}"
                );
            }
            assert!(!svg.contains("SourceFace"), "{layout}: {source_style}");
            assert!(!svg.contains("font-size:30px"), "{layout}: {source_style}");
            assert_eq!((applied, not_applicable, residuals), (2, 0, 0));
        }
    }
}

#[test]
fn opposing_root_and_flowchart_label_modes_keep_provider_title_ownership_and_portability() {
    let spec = DiagramThemeSpec::new()
        .with_typography(
            merman_render::diagram_theme::TypographySpec::default().with_family_style(
                merman_model::family::DiagramFamilyId::ER,
                ThemeTextStyle::default()
                    .with_font_stack(FontStack::single("ErTypedFace").expect("font stack"))
                    .with_font_size_px(24.0)
                    .expect("font size"),
            ),
        )
        .with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#123abc").expect("typed title color")),
            )),
        );
    let source_style = "style Orders color:#c0ffee,font-family:SourceFace,font-size:30px";
    for layout in LAYOUTS {
        for (root_html_labels, flowchart_html_labels) in [(false, true), (true, false)] {
            let config = json!({
                "layout": layout,
                "look": "classic",
                "htmlLabels": root_html_labels,
                "flowchart": {"htmlLabels": flowchart_html_labels},
                "themeVariables": {"titleColor": false}
            });
            let title_html_labels = if *layout == "swimlane" {
                flowchart_html_labels
            } else {
                root_html_labels
            };
            let context = format!(
                "{layout}, root htmlLabels={root_html_labels}, flowchart.htmlLabels={flowchart_html_labels}"
            );
            for requirement in [
                ThemePortabilityRequirement::BestEffort,
                ThemePortabilityRequirement::RequirePortable,
            ] {
                let result = render_cluster_with_spec(
                    config.clone(),
                    source_style,
                    spec.clone(),
                    requirement,
                );
                if title_html_labels
                    && matches!(requirement, ThemePortabilityRequirement::RequirePortable)
                {
                    let error = result.expect_err("visible source title fonts remain nonportable");
                    assert_eq!(
                        error.unverified_family_theme(),
                        Some((merman_model::family::DiagramFamilyId::ER, 2)),
                        "{context}"
                    );
                    continue;
                }
                let (svg, applied, not_applicable, residuals) =
                    result.expect("render provider-specific title ownership control");
                let document = roxmltree::Document::parse(&svg).expect("valid ER SVG");
                let cluster = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("merman-Orders"))
                    .expect("authored Orders cluster");
                let label = cluster
                    .descendants()
                    .find(|node| {
                        node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_whitespace()
                                .any(|class| class == "cluster-label")
                        })
                    })
                    .expect("provider cluster title label");
                assert_eq!(
                    label
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    title_html_labels,
                    "{context}"
                );
                if title_html_labels {
                    let span = label
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("span")
                                && node.attribute("class") == Some("nodeLabel")
                        })
                        .expect("HTML title style owner");
                    assert_eq!(
                        span.attribute("style"),
                        Some(
                            "color:#c0ffee !important; font-family:SourceFace !important; font-size:30px !important"
                        ),
                        "{context}"
                    );
                    assert_eq!((applied, not_applicable, residuals), (0, 1, 2), "{context}");
                } else {
                    let text = label
                        .descendants()
                        .find(|node| node.has_tag_name("text"))
                        .expect("SVG title style owner");
                    assert_eq!(
                        text.attribute("style"),
                        Some("color:#123abc;fill:#123abc"),
                        "{context}"
                    );
                    assert!(!svg.contains("SourceFace"), "{context}");
                    assert!(!svg.contains("font-size:30px"), "{context}");
                    assert_eq!((applied, not_applicable, residuals), (3, 0, 0), "{context}");
                }
            }
        }
    }
}

#[allow(dead_code)]
mod common;
