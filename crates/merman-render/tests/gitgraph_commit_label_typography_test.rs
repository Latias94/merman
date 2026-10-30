#![cfg(feature = "diagram-git-graph")]

use std::sync::{Arc, Mutex};

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfile, TextMeasurementProfileIdentity,
    TextMeasurementResultKind,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use serde_json::{Value, json};

const SOURCE: &str = r#"---
title: Graph title
---
gitGraph
  commit id: "CommitLabel" msg: "Message only" tag: "TagLabel"
"#;

#[derive(Default)]
struct WeightSensitiveMeasurer {
    requests: Mutex<Vec<(String, TextMeasurementOperation, TextStyle)>>,
}

impl WeightSensitiveMeasurer {
    fn metrics(
        &self,
        text: &str,
        operation: TextMeasurementOperation,
        style: &TextStyle,
    ) -> TextMetrics {
        self.requests
            .lock()
            .unwrap()
            .push((text.to_owned(), operation, style.clone()));
        let (width, height) = match style.font_weight.as_deref() {
            Some("700") => (80.0, 18.0),
            Some("600") => (60.0, 15.0),
            Some("725") => (90.0, 20.0),
            _ => (30.0, 9.0),
        };
        TextMetrics {
            width,
            height,
            line_count: 1,
        }
    }
}

impl HostTextMeasurer for WeightSensitiveMeasurer {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let metrics = self.metrics(request.text, request.operation, request.style);
        Ok(Some(match request.operation.required_result_kind() {
            TextMeasurementResultKind::Metrics => HostTextMeasurement::Metrics(metrics),
            TextMeasurementResultKind::Length => {
                HostTextMeasurement::Length(match request.operation {
                    TextMeasurementOperation::RawBBoxHeight
                    | TextMeasurementOperation::SimpleBBoxHeight
                    | TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                    TextMeasurementOperation::CreateTextBBoxYOffset
                    | TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                    _ => metrics.width,
                })
            }
            TextMeasurementResultKind::HorizontalExtents => {
                HostTextMeasurement::HorizontalExtents {
                    left: metrics.width / 2.0,
                    right: metrics.width / 2.0,
                }
            }
            TextMeasurementResultKind::WrappedWithRawWidth => {
                HostTextMeasurement::WrappedWithRawWidth {
                    metrics,
                    raw_width: Some(metrics.width),
                }
            }
        }))
    }
}

impl TextMeasurer for WeightSensitiveMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.metrics(text, TextMeasurementOperation::Measure, style)
    }

    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.metrics(text, TextMeasurementOperation::RawBBoxWidth, style)
            .width
    }

    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.metrics(text, TextMeasurementOperation::SimpleBBoxHeight, style)
            .height
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.metrics(text, TextMeasurementOperation::TspanBBoxHeight, style)
            .height
    }
}

fn render(
    theme: &str,
    weight: Option<Value>,
    font_size: &str,
    host: bool,
) -> (String, Arc<WeightSensitiveMeasurer>) {
    let mut config = json!({
        "theme": theme,
        "gitGraph": { "rotateCommitLabel": false },
        "themeVariables": { "commitLabelFontSize": font_size }
    });
    if let Some(weight) = weight {
        config["themeVariables"]["noteFontWeight"] = weight;
    }
    render_config(config, host)
}

fn render_config(config: Value, host: bool) -> (String, Arc<WeightSensitiveMeasurer>) {
    render_source(config, host, SOURCE)
}

fn render_source(
    config: Value,
    host: bool,
    source: &str,
) -> (String, Arc<WeightSensitiveMeasurer>) {
    let parsed = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let measurer = Arc::new(WeightSensitiveMeasurer::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("gitgraph-commit-label-weight").unwrap(),
        "1",
    )
    .unwrap();
    let policy = if host {
        TextMeasurementPolicy::host_display(identity, measurer.clone(), TextMeasurementPhase::ALL)
    } else {
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, measurer.clone()))
    };
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(policy);
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("git-weight".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_owned();
    (svg, measurer)
}

fn rule<'a>(svg: &'a str, selector: &str) -> &'a str {
    svg.split_once(&format!("#git-weight {selector}{{"))
        .unwrap()
        .1
        .split_once('}')
        .unwrap()
        .0
}

fn assert_geometry(svg: &str, class: &str, width: f64, height: f64) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let background = document
        .descendants()
        .find(|node| node.attribute("class") == Some(class))
        .unwrap();
    assert_eq!(
        background
            .attribute("width")
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        width
    );
    assert_eq!(
        background
            .attribute("height")
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        height
    );
}

fn assert_commit_weight(measurer: &WeightSensitiveMeasurer, expected: Option<&str>) {
    let requests = measurer.requests.lock().unwrap();
    let labels: Vec<_> = requests
        .iter()
        .filter(|(text, _, _)| text == "CommitLabel")
        .collect();
    assert!(!labels.is_empty(), "the commit label must be measured");
    assert!(
        labels
            .iter()
            .all(|(_, _, style)| style.font_weight.as_deref() == expected),
        "expected {expected:?}: {labels:?}"
    );
}

#[test]
fn gitgraph_commit_label_weight_reaches_host_and_probe_geometry() {
    for host in [true, false] {
        for theme in ["neo", "redux", "redux-color"] {
            for (weight, expected, width, height) in [
                (json!("normal"), "400", 34.0, 13.0),
                (json!("bold"), "700", 84.0, 22.0),
                (json!("bold !important"), "700", 84.0, 22.0),
                (json!(600), "600", 64.0, 19.0),
                (json!("725"), "725", 94.0, 24.0),
            ] {
                let css_weight = weight
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| weight.to_string());
                for (size, height_operation) in [
                    ("10px", TextMeasurementOperation::SimpleBBoxHeight),
                    ("18px", TextMeasurementOperation::TspanBBoxHeight),
                ] {
                    let (svg, measurer) = render(theme, Some(weight.clone()), size, host);
                    assert!(
                        rule(&svg, ".commit-label").contains(&format!("font-weight:{css_weight};"))
                    );
                    assert_geometry(&svg, "commit-label-bkg", width, height);
                    let requests = measurer.requests.lock().unwrap();
                    for operation in [TextMeasurementOperation::RawBBoxWidth, height_operation] {
                        assert!(
                            requests.iter().any(|(text, actual_operation, style)| {
                                text == "CommitLabel"
                                    && *actual_operation == operation
                                    && style.font_weight.as_deref() == Some(expected)
                            }),
                            "{theme}, host={host}, {weight}: {requests:?}"
                        );
                    }
                    for text in ["TagLabel", "Graph title"] {
                        assert!(
                            requests.iter().any(|(measured, _, style)| {
                                measured == text && style.font_weight.is_none()
                            }),
                            "{theme}: {text} must retain its own style: {requests:?}"
                        );
                    }
                    let branch_probes: Vec<_> = requests
                        .iter()
                        .filter(|(text, operation, _)| {
                            text == "main" && *operation == TextMeasurementOperation::Measure
                        })
                        .collect();
                    assert!(
                        !branch_probes.is_empty(),
                        "the default branch must be measured"
                    );
                    assert!(
                        branch_probes
                            .iter()
                            .all(|(_, _, style)| style.font_weight.is_none()),
                        "branch prelayout must retain normal weight: {branch_probes:?}"
                    );
                    if theme != "neo" {
                        for operation in [
                            if host {
                                TextMeasurementOperation::TspanBBoxWidth
                            } else {
                                TextMeasurementOperation::RawBBoxWidth
                            },
                            TextMeasurementOperation::TspanBBoxHeight,
                        ] {
                            assert!(
                                requests.iter().any(|(text, actual_operation, style)| {
                                    text == "main"
                                        && *actual_operation == operation
                                        && style.font_weight.as_deref() == Some(expected)
                                }),
                                "final branch bbox must use its numbered class: {requests:?}"
                            );
                        }
                        assert_geometry(&svg, "branchLabelBkg label0", width + 30.0, height + 12.0);
                    } else {
                        assert!(
                            requests
                                .iter()
                                .filter(|(text, _, _)| text == "main")
                                .all(|(_, _, style)| style.font_weight.is_none())
                        );
                    }
                    if theme == "neo" {
                        assert!(!rule(&svg, ".branch-label0").contains("font-weight:"));
                    } else {
                        assert!(
                            rule(&svg, ".branch-label0")
                                .contains(&format!("font-weight:{css_weight};"))
                        );
                    }
                    assert!(!rule(&svg, ".tag-label").contains("font-weight:"));
                    assert!(!svg.contains(">Message only</text>"));
                    assert!(!requests.iter().any(|(text, _, _)| text == "Message only"));
                }
            }
        }
    }
}

#[test]
fn gitgraph_commit_label_weight_preserves_classic_and_theme_defaults() {
    for host in [true, false] {
        for theme in ["default", "base", "dark", "forest", "neutral"] {
            let (svg, measurer) = render(theme, Some(json!("bold")), "10px", host);
            assert!(!rule(&svg, ".commit-label").contains("font-weight:"));
            assert_geometry(&svg, "commit-label-bkg", 34.0, 13.0);
            assert_commit_weight(&measurer, None);
        }
        for (theme, css_weight, measured, width, height) in [
            ("neo", "normal", "400", 34.0, 13.0),
            ("neo-dark", "normal", "400", 34.0, 13.0),
            ("redux", "600", "600", 64.0, 19.0),
            ("redux-dark", "600", "600", 64.0, 19.0),
            ("redux-color", "600", "600", 64.0, 19.0),
            ("redux-dark-color", "600", "600", 64.0, 19.0),
        ] {
            let (svg, measurer) = render(theme, None, "10px", host);
            assert!(rule(&svg, ".commit-label").contains(&format!("font-weight:{css_weight};")));
            assert_geometry(&svg, "commit-label-bkg", width, height);
            assert_commit_weight(&measurer, Some(measured));
        }
    }
}

#[test]
fn gitgraph_commit_label_dynamic_or_unknown_weight_is_not_replaced_by_normal() {
    for host in [true, false] {
        for weight in ["var(--commit-weight)", "calc(400 + 300)", "unknown-weight"] {
            let (svg, measurer) = render("redux", Some(json!(weight)), "10px", host);
            assert!(rule(&svg, ".commit-label").contains(&format!("font-weight:{weight};")));
            assert_commit_weight(&measurer, Some(weight));
        }
    }
}

#[test]
fn gitgraph_commit_label_unmeasurable_weight_cannot_certify_typed_typography() {
    use merman_model::family::DiagramFamilyId;
    use merman_render::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemePortabilityRequirement,
        ThemeTextStyle, TypographySpec,
    };

    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("GitGraphWeight").unwrap())
        .with_font_size_px(23.0)
        .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::GIT_GRAPH, typography),
        ))
        .unwrap();
    for show_commit_label in [true, false] {
        for weight in ["var(--commit-weight)", "calc(400 + 300)", "unknown-weight"] {
            for requirement in [
                ThemePortabilityRequirement::RequirePortable,
                ThemePortabilityRequirement::BestEffort,
            ] {
                let engine = MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(json!({
                        "theme": "redux",
                        "gitGraph": { "showCommitLabel": show_commit_label },
                        "themeVariables": { "noteFontWeight": weight }
                    })),
                );
                let parsed = common::install_parse_compatibility(&theme, engine)
                    .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(requirement)
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let result =
                    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                        .and_then(|artifact| {
                            artifact.render_svg(
                                &SvgRenderOptions {
                                    diagram_id: Some("git-weight".to_owned()),
                                    ..Default::default()
                                },
                                &SvgDebugOptions::default(),
                            )
                        });
                if requirement == ThemePortabilityRequirement::RequirePortable {
                    let error = result
                        .err()
                        .expect("unresolved commit weight must fail closed");
                    assert_eq!(
                        error.unverified_family_theme(),
                        Some((DiagramFamilyId::GIT_GRAPH, 2)),
                        "{weight}: {error:?}"
                    );
                } else {
                    let rendered = result.expect("best effort preserves unresolved Mermaid CSS");
                    assert!(
                        rule(rendered.svg(), ".commit-label")
                            .contains(&format!("font-weight:{weight};"))
                    );
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(evidence.theme_residual_count(), 2);
                    assert_eq!(evidence.applied_count(), 0);
                }
            }
        }
    }
}

#[test]
fn gitgraph_measurement_uses_the_css_bound_font_family() {
    for host in [true, false] {
        let (svg, measurer) = render_config(
            json!({
                "theme": "redux",
                "fontFamily": "RootFallback, serif",
                "themeVariables": {
                    "fontFamily": "ThemeOverride, sans-serif",
                    "noteFontWeight": "bold"
                }
            }),
            host,
        );
        assert!(svg.contains("font-family:ThemeOverride,sans-serif;"));
        let requests = measurer.requests.lock().unwrap();
        assert!(!requests.is_empty());
        assert!(
            requests
                .iter()
                .all(|(_, _, style)| style.font_family.as_deref()
                    == Some("ThemeOverride,sans-serif")),
            "{requests:?}"
        );
    }
}

#[test]
fn gitgraph_branch_weight_requires_its_numbered_css_rule() {
    const BRANCHES: &str =
        "gitGraph\ncommit id: \"A\"\nbranch side\ncheckout side\ncommit id: \"B\"\n";
    for host in [true, false] {
        for limit in [1, 2] {
            let (svg, measurer) = render_source(
                json!({
                    "theme": "redux",
                    "gitGraph": {"showCommitLabel": false},
                    "themeVariables": {"THEME_COLOR_LIMIT": limit, "noteFontWeight": "bold"}
                }),
                host,
                BRANCHES,
            );
            let expected = (limit > 1).then_some("700");
            let requests = measurer.requests.lock().unwrap();
            assert!(
                requests
                    .iter()
                    .any(|(text, operation, style)| text == "side"
                        && *operation == TextMeasurementOperation::TspanBBoxHeight
                        && style.font_weight.as_deref() == expected),
                "limit={limit}: {requests:?}"
            );
            assert_eq!(svg.contains("#git-weight .branch-label1{"), limit > 1);
            assert_geometry(&svg, "branchLabelBkg label0", 114.0, 34.0);
            let (width, height) = if limit > 1 {
                (114.0, 34.0)
            } else {
                (64.0, 25.0)
            };
            assert_geometry(&svg, "branchLabelBkg label1", width, height);
        }
    }
}

#[allow(dead_code)]
mod common;
