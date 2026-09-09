mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use merman_render::{DiagramFamilyId, LayoutOptions};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

fn requirement_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile Requirement fill theme")
}

fn requirement_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default().with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile Requirement stroke theme")
}

fn requirement_fill_and_stroke_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default()
                            .with_fill(fill)
                            .with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile Requirement fill and stroke theme")
}

fn requirement_fill_with_ordinal_palette_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Requirement,
                            ThemeStylePatch::default().with_fill(fill),
                        )
                        .for_family(DiagramFamilyId::REQUIREMENT),
                    )
                    .with_ordinal_palette(
                        ThemeTarget::Requirement,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#123456").expect("valid palette color")
                        ])
                        .expect("non-empty Requirement palette"),
                    ),
            ),
        )
        .expect("compile Requirement fill and ordinal palette theme")
}

fn requirement_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::REQUIREMENT, typography),
        ))
        .expect("compile Requirement typography theme")
}

fn requirement_ordinal_palette_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Requirement,
                    OrdinalPalette::new([
                        ThemeColorValue::parse("#123456").expect("valid palette color")
                    ])
                    .expect("non-empty Requirement palette"),
                ),
            ),
        )
        .expect("compile Requirement ordinal palette theme")
}

fn render_requirement_with_theme(source: &str, theme: &DiagramTheme) -> family::RenderedFamilySvg {
    render_requirement_with_theme_and_engine(source, theme, Engine::new())
}

fn render_requirement_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    render_requirement_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn render_requirement_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> family::RenderedFamilySvg {
    try_render_requirement_with_theme_requirement_and_environment(
        source,
        theme,
        engine,
        portability,
        RenderEnvironment::deterministic(),
    )
    .expect("render themed Requirement")
}

fn render_requirement_with_theme_requirement_and_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> family::RenderedFamilySvg {
    try_render_requirement_with_theme_requirement_and_environment(
        source,
        theme,
        engine,
        portability,
        environment,
    )
    .expect("render themed Requirement")
}

fn try_render_requirement_with_theme_requirement_and_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Requirement")
        .expect("detect themed Requirement");
    let session = environment
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Requirement session");

    family::prepare(parsed, &LayoutOptions::default(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("requirement-theme".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn render_requirement_without_theme(source: &str) -> family::RenderedFamilySvg {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Requirement")
        .expect("detect Requirement");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin Requirement session");

    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Requirement")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("requirement-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Requirement")
}

fn terminal_fill_for(document: &roxmltree::Document<'_>, node_id: &str) -> String {
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Requirement node group {node_id}"));
    requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("stroke") == Some("none")
                && node.attribute("stroke-width") == Some("0")
        })
        .and_then(|path| path.attribute("fill"))
        .unwrap_or_else(|| panic!("missing terminal fill path for {node_id}"))
        .to_string()
}

fn terminal_stroke_for(document: &roxmltree::Document<'_>, node_id: &str) -> String {
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Requirement node group {node_id}"));
    requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("fill") == Some("none")
                && node.attribute("stroke-width") == Some("1.3")
                && node.ancestors().any(|ancestor| {
                    ancestor.attribute("class") == Some("basic label-container outer-path")
                })
        })
        .and_then(|path| path.attribute("stroke"))
        .unwrap_or_else(|| panic!("missing terminal stroke path for {node_id}"))
        .to_string()
}

fn terminal_stroke_geometry_for(document: &roxmltree::Document<'_>, node_id: &str) -> String {
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Requirement node group {node_id}"));
    requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("fill") == Some("none")
                && node.attribute("stroke-width") == Some("1.3")
                && node.ancestors().any(|ancestor| {
                    ancestor.attribute("class") == Some("basic label-container outer-path")
                })
        })
        .and_then(|path| path.attribute("d"))
        .unwrap_or_else(|| panic!("missing terminal stroke geometry for {node_id}"))
        .to_string()
}

fn requirement_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Requirement SVG");
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap_or_else(|| panic!("missing Requirement stylesheet"))
        .to_string()
}

fn requirement_source() -> &'static str {
    r#"requirementDiagram
  requirement req1 {
    id: 1
    text: Themed requirement
    risk: high
    verifymethod: analysis
  }
"#
}

fn requirement_svg_label_source() -> &'static str {
    r#"requirementDiagram
  requirement req1 {
    id: "R-1"
    text: "**Strong** and *emphasis* &amp; entity"
    risk: high
    verifymethod: analysis
  }
  element elem1 {
    type: simulation
    docref: "*Document* &amp; reference"
  }
  req1 - satisfies -> elem1
  req1 - traces -> req1
  style req1 color:#123456,fill:#abcdef
"#
}

#[test]
fn requirement_svg_labels_cover_nodes_relationships_and_self_loop_anchors() {
    let rendered = render_requirement_with_theme_and_engine(
        requirement_svg_label_source(),
        &requirement_typography_theme(ThemeTextStyle::default()),
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": false
        }))),
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid SVG label document");
    for (id, expected_lines) in [
        ("requirement-theme-req1", 6),
        ("requirement-theme-elem1", 4),
    ] {
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .expect("semantic node");
        assert_eq!(
            node.descendants()
                .filter(|node| node.has_tag_name("foreignObject"))
                .count(),
            0,
            "{id}"
        );
        let labels: Vec<_> = node
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect();
        assert_eq!(
            labels.len(),
            expected_lines,
            "each populated line of {id} is SVG text"
        );
        assert!(
            labels
                .iter()
                .all(|label| label.descendants().any(|node| node.has_tag_name("tspan")))
        );
    }
    let req = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-req1"))
        .unwrap();
    let text = req
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect::<String>();
    assert!(text.contains("Strong and emphasis & entity"), "{text}");
    for (word, property, value) in [
        ("Strong", "font-weight", "bold"),
        ("emphasis", "font-style", "italic"),
    ] {
        assert!(
            req.descendants().any(|node| node.has_tag_name("tspan")
                && node.text().is_some_and(|text| text.trim() == word)
                && node.attribute(property) == Some(value)),
            "missing Markdown {word}"
        );
    }
    assert!(
        req.descendants()
            .filter(|node| node.has_tag_name("text"))
            .all(|label| {
                label
                    .attribute("style")
                    .is_some_and(|style| style.contains("fill:#123456"))
                    && label.descendants().any(|node| {
                        node.has_tag_name("tspan")
                            && node
                                .attribute("style")
                                .is_some_and(|style| style.contains("color:#123456"))
                    })
            })
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#abcdef"
    );
    let header = req
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("req1"))
        .unwrap();
    assert!(
        header
            .attribute("style")
            .is_some_and(|style| style.contains("font-weight: bold"))
    );
    assert!(
        req.descendants()
            .filter(|node| node.has_tag_name("text"))
            .all(|node| node.attribute("text-anchor").is_none())
    );
    let edge_labels: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("data-id").is_some())
        .collect();
    assert!(!edge_labels.is_empty());
    for label in edge_labels {
        assert!(
            !label
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
        assert!(label.descendants().any(|node| node.has_tag_name("text")));
    }
    for id in ["req1---req1---1", "req1---req1---2"] {
        let anchor = document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .expect("self-loop anchor");
        assert!(
            !anchor
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
        assert!(anchor.descendants().any(|node| node.has_tag_name("text")));
    }
}

#[test]
fn requirement_default_labels_match_explicit_html_labels() {
    let theme = requirement_typography_theme(ThemeTextStyle::default());
    let default = render_requirement_with_theme(requirement_svg_label_source(), &theme);
    let explicit = render_requirement_with_theme_and_engine(
        requirement_svg_label_source(),
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(
            serde_json::json!({"htmlLabels": true}),
        )),
    );
    assert_eq!(default.svg(), explicit.svg());
    let document = roxmltree::Document::parse(default.svg()).expect("valid default HTML labels");
    let req = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-req1"))
        .unwrap();
    assert_eq!(
        req.descendants()
            .filter(|node| node.has_tag_name("foreignObject"))
            .count(),
        6
    );
}

#[derive(Debug)]
struct RequirementSvgModeProbe {
    calls: Arc<Mutex<Vec<(WrapMode, TextStyle)>>>,
}

impl TextMeasurer for RequirementSvgModeProbe {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        TextMetrics {
            width: text.len() as f64 * 5.0,
            height: style.font_size,
            line_count: 1,
        }
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        _width: Option<f64>,
        mode: WrapMode,
    ) -> TextMetrics {
        self.calls.lock().unwrap().push((mode, style.clone()));
        self.measure(text, style)
    }
}

#[test]
fn requirement_svg_labels_measure_with_svg_mode_and_portable_typed_typography() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let stack = FontStack::new(["Requirement SVG", "sans-serif"]).unwrap();
    let expected_font = stack.as_css();
    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(stack)
            .with_font_size_px(24.0)
            .unwrap(),
    );
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.requirement-svg-mode").unwrap(),
            "test",
        )
        .unwrap(),
        Arc::new(RequirementSvgModeProbe {
            calls: Arc::clone(&calls),
        }),
    );
    let rendered = render_requirement_with_theme_requirement_and_environment(
        requirement_svg_label_source(),
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(
            serde_json::json!({"htmlLabels": false}),
        )),
        ThemePortabilityRequirement::RequirePortable,
        RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
    );
    let calls = calls.lock().unwrap();
    assert!(!calls.is_empty());
    assert_eq!(
        calls
            .iter()
            .filter(|(mode, _)| *mode != WrapMode::SvgLike)
            .count(),
        0,
        "all wrapped Requirement requests must use SVG mode"
    );
    assert!(calls.iter().all(|(_, style)| style.font_family.as_deref()
        == Some(expected_font.as_str())
        && style.font_size == 24.0));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

fn same_named_requirement_and_element_source() -> &'static str {
    r#"requirementDiagram
  requirement shared {
    id: R-1
    text: Requirement owner
    risk: low
    verifymethod: analysis
  }
  element shared {
    type: simulation
    docref: Element owner
  }
"#
}

#[derive(Debug)]
struct RequirementTypedFontProbeMeasurer {
    expected_font_family: String,
    typed_calls: Arc<AtomicUsize>,
    sans_serif_fallback_calls: Arc<AtomicUsize>,
}

#[derive(Debug)]
struct RequirementTypedFontSizeProbeMeasurer {
    expected_font_size: f64,
    typed_size_calls: Arc<AtomicUsize>,
    calculation_size_calls: Arc<AtomicUsize>,
    unexpected_size_calls: Arc<AtomicUsize>,
}

impl TextMeasurer for RequirementTypedFontSizeProbeMeasurer {
    fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
        if (style.font_size - self.expected_font_size).abs() < f64::EPSILON {
            self.typed_size_calls.fetch_add(1, Ordering::Relaxed);
        } else if (style.font_size - 10.0).abs() < f64::EPSILON {
            self.calculation_size_calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.unexpected_size_calls.fetch_add(1, Ordering::Relaxed);
        }
        TextMetrics {
            width: 100.0,
            height: style.font_size,
            line_count: 1,
        }
    }
}

impl TextMeasurer for RequirementTypedFontProbeMeasurer {
    fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
        match style.font_family.as_deref() {
            Some(actual) if actual == self.expected_font_family => {
                self.typed_calls.fetch_add(1, Ordering::Relaxed);
            }
            Some("sans-serif") => {
                self.sans_serif_fallback_calls
                    .fetch_add(1, Ordering::Relaxed);
            }
            actual => panic!(
                "Requirement layout measurement must use the resolved font winner or Mermaid's explicit sans-serif fallback probe: {actual:?}"
            ),
        }
        TextMetrics {
            width: 100.0,
            height: style.font_size,
            line_count: 1,
        }
    }
}

#[test]
fn requirement_source_fonts_reach_node_measurement_and_retire_inherited_claims() {
    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("RequirementTyped").unwrap())
            .with_font_size_px(24.0)
            .unwrap(),
    );
    for html_labels in [false, true] {
        for source_style in [
            "style req1 font-family:SourceFont,font-size:32px",
            "classDef sourceFont font-family:SourceFont,font-size:32px\nclass req1 sourceFont",
        ] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let profile = TextMeasurementProfile::new(
                TextMeasurementProfileIdentity::new(
                    MeasurementProfileId::new("test.requirement-source-fonts").unwrap(),
                    "test",
                )
                .unwrap(),
                Arc::new(RequirementSvgModeProbe {
                    calls: Arc::clone(&calls),
                }),
            );
            let rendered = render_requirement_with_theme_requirement_and_environment(
                &format!("{}\n{source_style}\n", requirement_source()),
                &theme,
                Engine::new().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({"htmlLabels": html_labels}),
                )),
                ThemePortabilityRequirement::RequirePortable,
                RenderEnvironment::deterministic()
                    .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
            );
            let calls = calls.lock().unwrap();
            assert!(!calls.is_empty());
            assert!(
                calls.iter().all(
                    |(_, style)| style.font_family.as_deref() == Some("SourceFont")
                        && style.font_size == 32.0
                ),
                "{html_labels}/{source_style}: {calls:?}"
            );
            assert!(
                calls
                    .iter()
                    .any(|(_, style)| style.font_weight.as_deref() == Some("bold"))
            );
            assert!(calls.iter().any(|(_, style)| style.font_weight.is_none()));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(document.descendants().any(|node| {
                node.attribute("style").is_some_and(|style| {
                    style.contains("font-family:SourceFont !important")
                        && style.contains("font-size:32px !important")
                })
            }));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 2);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn requirement_source_font_faces_reach_measurement_and_override_name_defaults() {
    #[derive(Debug)]
    struct FaceProbe(Arc<Mutex<Vec<TextStyle>>>);

    impl TextMeasurer for FaceProbe {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.lock().unwrap().push(style.clone());
            TextMetrics {
                width: text.len() as f64 * style.font_size,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("RequirementTyped").unwrap())
            .with_font_size_px(24.0)
            .unwrap(),
    );
    for html_labels in [false, true] {
        for (declarations, weight, face) in [
            ("font-weight:500,font-style:italic", "500", "italic"),
            ("font-weight:700,font-style:OBLIQUE", "700", "oblique"),
            (
                "font-weight:bold!important,font-weight:normal,font-style:italic,font-style:normal",
                "normal",
                "normal",
            ),
        ] {
            for source_style in [
                format!("style req1 font-size:32px,{declarations}"),
                format!("classDef sourceFace font-size:32px,{declarations}\nclass req1 sourceFace"),
                format!(
                    "classDef earlierFace font-weight:900,font-style:normal\nclass req1 earlierFace\nstyle req1 font-size:32px,{declarations}"
                ),
                format!(
                    "style req1 font-weight:900,font-style:normal\nclassDef laterFace font-size:32px,{declarations}\nclass req1 laterFace"
                ),
            ] {
                let calls = Arc::new(Mutex::new(Vec::new()));
                let profile = TextMeasurementProfile::new(
                    TextMeasurementProfileIdentity::new(
                        MeasurementProfileId::new("test.requirement-source-faces").unwrap(),
                        "test",
                    )
                    .unwrap(),
                    Arc::new(FaceProbe(Arc::clone(&calls))),
                );
                let rendered = render_requirement_with_theme_requirement_and_environment(
                    &format!("{}\n{source_style}\n", requirement_source()),
                    &theme,
                    Engine::new().with_site_config(MermaidConfig::from_value(
                        serde_json::json!({"htmlLabels": html_labels}),
                    )),
                    ThemePortabilityRequirement::RequirePortable,
                    RenderEnvironment::deterministic()
                        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
                );
                let calls = calls.lock().unwrap();
                let node_calls = calls.iter().filter(|style| style.font_size == 32.0);
                assert!(node_calls.clone().count() > 0);
                assert!(
                    node_calls.clone().all(|style| {
                        style.font_weight.as_deref() == Some(weight)
                            && style.font_style.as_deref() == Some(face)
                    }),
                    "{html_labels}/{source_style}: {calls:?}"
                );
                if !html_labels {
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let multiline_labels = document
                        .descendants()
                        .filter(|node| {
                            node.has_tag_name("text")
                                && node
                                    .children()
                                    .filter(|row| row.has_tag_name("tspan"))
                                    .count()
                                    > 1
                        })
                        .collect::<Vec<_>>();
                    assert!(
                        !multiline_labels.is_empty(),
                        "the probe must force SVG wrapping"
                    );
                    for label in multiline_labels {
                        for word in label
                            .descendants()
                            .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
                        {
                            let style = word.attribute("style").unwrap_or("").to_ascii_lowercase();
                            assert!(
                                style.contains(&format!("font-weight:{weight} !important"))
                                    && style.contains(&format!("font-style:{face} !important")),
                                "every wrapped word must retain the source face: {source_style}/{word:?}"
                            );
                        }
                    }
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn requirement_svg_source_faces_override_markdown_but_html_keeps_child_faces() {
    #[derive(Debug)]
    struct FaceProbe(Arc<Mutex<Vec<TextStyle>>>);

    impl TextMeasurer for FaceProbe {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.lock().unwrap().push(style.clone());
            TextMetrics {
                width: text.len() as f64 * style.font_size,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    let theme = requirement_typography_theme(
        ThemeTextStyle::default().with_font_stack(FontStack::single("RequirementTyped").unwrap()),
    );
    for html_labels in [false, true] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.requirement-markdown-source-faces").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(FaceProbe(Arc::clone(&calls))),
        );
        let source = format!(
            "{}\nstyle req1 font-size:32px,font-weight:500,font-style:normal\n",
            requirement_source().replace("Themed requirement", "\"**Strong** and *emphasis*\"")
        );
        let rendered = render_requirement_with_theme_requirement_and_environment(
            &source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"htmlLabels": html_labels}),
            )),
            ThemePortabilityRequirement::RequirePortable,
            RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
        );
        let calls = calls.lock().unwrap();
        let node_calls = calls.iter().filter(|style| style.font_size == 32.0);
        assert!(node_calls.clone().count() > 0);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        if html_labels {
            assert!(
                node_calls
                    .clone()
                    .any(|style| style.font_weight.as_deref() == Some("700"))
            );
            assert!(
                node_calls
                    .clone()
                    .any(|style| style.font_style.as_deref() == Some("italic"))
            );
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("strong"))
            );
            assert!(document.descendants().any(|node| node.has_tag_name("em")));
        } else {
            assert!(
                node_calls.clone().all(|style| {
                    style.font_weight.as_deref() == Some("500")
                        && style.font_style.as_deref() == Some("normal")
                }),
                "{calls:?}"
            );
            let words = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
                .collect::<Vec<_>>();
            assert!(
                words
                    .iter()
                    .any(|word| word.attribute("font-weight") == Some("bold"))
            );
            assert!(
                words
                    .iter()
                    .any(|word| word.attribute("font-style") == Some("italic"))
            );
            assert!(
                words
                    .iter()
                    .all(|word| word.attribute("style").is_some_and(|style| {
                        style.contains("font-weight:500 !important")
                            && style.contains("font-style:normal !important")
                    }))
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn requirement_source_font_evidence_is_property_local_and_fails_closed() {
    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("RequirementTyped").unwrap())
            .with_font_size_px(24.0)
            .unwrap(),
    );
    for html_labels in [false, true] {
        for (style, applied, not_applicable, residual) in [
            ("font-family:SourceFont", 1, 1, 0),
            ("font-size:32px", 1, 1, 0),
            ("font-size:0", 0, 2, 0),
            ("font-family:var(--source)", 1, 0, 1),
            ("font-size:calc(1em)", 1, 0, 1),
            ("font-weight:bolder", 0, 0, 2),
            ("font-weight:lighter", 0, 0, 2),
            ("font-weight:var(--weight)", 0, 0, 2),
            ("font-style:var(--style)", 0, 0, 2),
            ("font-style:inherit", 0, 0, 2),
            ("font-style:unset", 0, 0, 2),
            ("font-weight:inherit", 0, 0, 2),
            ("font-weight:bold,font-weight:var(--weight)", 0, 0, 2),
            ("font-weight:var(--weight),font-weight:normal", 2, 0, 0),
        ] {
            let source = format!("{}\nstyle req1 {style}\n", requirement_source());
            let engine = || {
                Engine::new().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({"htmlLabels": html_labels}),
                ))
            };
            let rendered = render_requirement_with_theme_requirement(
                &source,
                &theme,
                engine(),
                ThemePortabilityRequirement::BestEffort,
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), applied, "{html_labels}/{style}");
            assert_eq!(
                evidence.not_applicable_count(),
                not_applicable,
                "{html_labels}/{style}"
            );
            assert_eq!(
                evidence.theme_residual_count(),
                residual,
                "{html_labels}/{style}"
            );
            let strict = try_render_requirement_with_theme_requirement_and_environment(
                &source,
                &theme,
                engine(),
                ThemePortabilityRequirement::RequirePortable,
                RenderEnvironment::deterministic(),
            );
            if residual == 0 {
                strict.expect("static source typography is measurable");
            } else {
                let error = strict
                    .err()
                    .expect("unknown source typography must fail closed");
                assert_eq!(
                    error.unverified_family_theme(),
                    Some((DiagramFamilyId::REQUIREMENT, residual))
                );
            }
        }
    }
}

#[test]
fn requirement_source_owned_nodes_do_not_suppress_inherited_edge_and_title_fonts() {
    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("RequirementTyped").unwrap())
            .with_font_size_px(24.0)
            .unwrap(),
    );
    let node = format!(
        "{}\nstyle req1 font-family:SourceFont,font-size:32px\n",
        requirement_source()
    );
    for source in [
        format!("---\ntitle: Inherited title\n---\n{node}"),
        format!(
            "{node}\nelement impl {{\n type: source\n}}\nstyle impl font-family:SourceFont,font-size:32px\nimpl - satisfies -> req1\n"
        ),
    ] {
        let rendered = render_requirement_with_theme_and_engine(&source, &theme, Engine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(document.descendants().any(|node| {
            matches!(
                node.attribute("class"),
                Some("requirementDiagramTitleText" | "edgeLabel")
            ) && node.descendants().any(|child| {
                child.is_text() && child.text().is_some_and(|text| !text.trim().is_empty())
            })
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 2);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn requirement_typed_font_stack_reaches_measurement_stylesheet_and_evidence() {
    let font_stack = FontStack::new(["Requirement Typed", "sans-serif"])
        .expect("valid Requirement typed font stack");
    let expected_font = font_stack.as_css();
    let theme = requirement_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.requirement-typed-font-probe").unwrap(),
        "test",
    )
    .unwrap();
    let typed_calls = Arc::new(AtomicUsize::new(0));
    let sans_serif_fallback_calls = Arc::new(AtomicUsize::new(0));
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(RequirementTypedFontProbeMeasurer {
                expected_font_family: expected_font.clone(),
                typed_calls: Arc::clone(&typed_calls),
                sans_serif_fallback_calls: Arc::clone(&sans_serif_fallback_calls),
            }),
        )),
    );
    let rendered = render_requirement_with_theme_requirement_and_environment(
        requirement_source(),
        &theme,
        legacy_init_theme_compat_engine(),
        ThemePortabilityRequirement::RequirePortable,
        environment,
    );
    assert!(
        typed_calls.load(Ordering::Relaxed) > 0,
        "Requirement layout must measure at least one label with the typed font stack"
    );
    assert!(
        sans_serif_fallback_calls.load(Ordering::Relaxed) > 0,
        "the Mermaid-compatible sans-serif comparison probe should remain observable"
    );
    let css = requirement_stylesheet(rendered.svg());
    for selector in [
        "#requirement-theme",
        "#requirement-theme svg",
        "#requirement-theme .label",
    ] {
        assert!(
            css.contains(&format!("{selector}{{font-family:{expected_font};")),
            "missing resolved Requirement font rule for {selector}: {css}"
        );
    }

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn requirement_typed_font_stack_respects_source_font_family_ownership() {
    let theme = requirement_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("Requirement Typed").expect("valid Requirement font stack"),
    ));
    let rendered = render_requirement_with_theme_and_engine(
        r##"%%{init: {"themeVariables": {"fontFamily": "Source Requirement"}}}%%
requirementDiagram
  requirement req1 {
    id: 1
    text: Source-owned family
    risk: low
    verifymethod: analysis
  }
"##,
        &theme,
        legacy_init_theme_compat_engine(),
    );
    let css = requirement_stylesheet(rendered.svg());
    assert!(
        css.contains("#requirement-theme{font-family:Source Requirement;")
            || css.contains("#requirement-theme svg{font-family:Source Requirement;"),
        "source-owned Requirement font must win in final CSS: {css}"
    );
    assert!(!css.contains("font-family:Requirement Typed"), "{css}");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn requirement_typed_font_size_reaches_measurement_stylesheet_and_evidence() {
    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Requirement typed font size"),
    );
    let typed_size_calls = Arc::new(AtomicUsize::new(0));
    let calculation_size_calls = Arc::new(AtomicUsize::new(0));
    let unexpected_size_calls = Arc::new(AtomicUsize::new(0));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.requirement-typed-font-size-probe").unwrap(),
        "test",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(RequirementTypedFontSizeProbeMeasurer {
                expected_font_size: 24.0,
                typed_size_calls: Arc::clone(&typed_size_calls),
                calculation_size_calls: Arc::clone(&calculation_size_calls),
                unexpected_size_calls: Arc::clone(&unexpected_size_calls),
            }),
        )),
    );
    let rendered = render_requirement_with_theme_requirement_and_environment(
        requirement_source(),
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "fontSize": 10
        }))),
        ThemePortabilityRequirement::RequirePortable,
        environment,
    );
    assert!(typed_size_calls.load(Ordering::Relaxed) > 0);
    assert!(calculation_size_calls.load(Ordering::Relaxed) > 0);
    assert_eq!(unexpected_size_calls.load(Ordering::Relaxed), 0);
    let css = requirement_stylesheet(rendered.svg());
    assert!(css.contains("#requirement-theme svg{font-family:"));
    assert!(
        css.contains("font-size:24px"),
        "typed Requirement size must reach CSS: {css}"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn requirement_mixed_base_typography_settles_each_property_independently() {
    let font_stack = FontStack::new(["Requirement Mixed", "sans-serif"])
        .expect("valid Requirement mixed font stack");
    let expected_font = font_stack.as_css();
    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(font_stack)
            .with_font_size_px(24.0)
            .expect("valid Requirement mixed typography"),
    );
    let typed_size_calls = Arc::new(AtomicUsize::new(0));
    let calculation_size_calls = Arc::new(AtomicUsize::new(0));
    let unexpected_size_calls = Arc::new(AtomicUsize::new(0));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.requirement-mixed-typography-probe").unwrap(),
        "test",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(RequirementTypedFontSizeProbeMeasurer {
                expected_font_size: 24.0,
                typed_size_calls: Arc::clone(&typed_size_calls),
                calculation_size_calls: Arc::clone(&calculation_size_calls),
                unexpected_size_calls: Arc::clone(&unexpected_size_calls),
            }),
        )),
    );
    let rendered = render_requirement_with_theme_requirement_and_environment(
        requirement_source(),
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "fontSize": 10
        }))),
        ThemePortabilityRequirement::RequirePortable,
        environment,
    );

    assert!(typed_size_calls.load(Ordering::Relaxed) > 0);
    assert!(calculation_size_calls.load(Ordering::Relaxed) > 0);
    assert_eq!(unexpected_size_calls.load(Ordering::Relaxed), 0);
    let css = requirement_stylesheet(rendered.svg());
    assert!(
        css.contains(&format!("font-family:{expected_font}")),
        "{css}"
    );
    assert!(css.contains("font-size:24px"), "{css}");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn requirement_typed_font_size_respects_explicit_theme_variable_ownership() {
    let theme = requirement_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Requirement typed font size"),
    );
    let rendered = render_requirement_with_theme_requirement(
        r##"%%{init: {"themeVariables": {"fontSize": "30px"}}}%%
requirementDiagram
  requirement req1 {
    id: 1
    text: Source-owned size
    risk: low
    verifymethod: analysis
  }
"##,
        &theme,
        legacy_init_theme_compat_engine(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let css = requirement_stylesheet(rendered.svg());
    assert!(css.contains("font-size:30px"), "{css}");
    assert!(!css.contains("font-size:24px"), "{css}");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn requirement_unsupported_typography_is_property_local_to_the_typed_font_stack() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::single("RequirementTyped").expect("valid Requirement font stack"),
        )
        .with_font_weight(700)
        .expect("valid unsupported Requirement font weight");
    let theme = requirement_typography_theme(typography);
    let rendered = render_requirement_with_theme_requirement(
        requirement_source(),
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );

    assert!(requirement_stylesheet(rendered.svg()).contains("font-family:RequirementTyped"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);

    let error = match try_render_requirement_with_theme_requirement_and_environment(
        requirement_source(),
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
        RenderEnvironment::deterministic(),
    ) {
        Ok(_) => panic!("strict Requirement must reject unsupported font weight"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::REQUIREMENT, 1))
    );
}

#[test]
fn same_named_requirement_and_element_render_one_default_fill_terminal() {
    let rendered = render_requirement_without_theme(same_named_requirement_and_element_source());
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid default Requirement SVG");
    let node_id = "requirement-theme-shared";

    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("id") == Some(node_id))
            .count(),
        1
    );
    assert_eq!(terminal_fill_for(&document, node_id), "#ECECFF");
}

#[test]
fn same_named_requirement_and_element_share_one_typed_fill_receipt() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid same-name Requirement fill"),
    );
    let rendered =
        render_requirement_with_theme(same_named_requirement_and_element_source(), &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid themed Requirement SVG");
    let node_id = "requirement-theme-shared";

    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("id") == Some(node_id))
            .count(),
        1
    );
    assert_eq!(terminal_fill_for(&document, node_id), "#123456");

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_fill_reaches_terminal_path_and_family_evidence() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Requirement fill"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (fill, expected_fill) in cases {
        let theme = requirement_fill_theme(fill);
        let rendered = render_requirement_with_theme(requirement_source(), &theme);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid themed Requirement SVG");
        assert_eq!(
            terminal_fill_for(&document, "requirement-theme-req1"),
            expected_fill
        );

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn requirement_stroke_reaches_terminal_path_and_family_evidence() {
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Requirement stroke"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    let mut expected_geometry = None;
    for (stroke, expected_stroke) in cases {
        let theme = requirement_stroke_theme(stroke);
        let rendered = render_requirement_with_theme(requirement_source(), &theme);
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid themed Requirement SVG");
        assert_eq!(
            terminal_stroke_for(&document, "requirement-theme-req1"),
            expected_stroke
        );
        let geometry = terminal_stroke_geometry_for(&document, "requirement-theme-req1");
        if let Some(expected_geometry) = &expected_geometry {
            assert_eq!(
                &geometry, expected_geometry,
                "Requirement stroke paint must not change terminal path geometry"
            );
        } else {
            expected_geometry = Some(geometry);
        }

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn requirement_typed_stroke_remains_the_neo_terminal_winner() {
    let theme = requirement_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid Neo Requirement stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "neo"
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Neo themed Requirement SVG");
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-req1"))
        .expect("Neo Requirement node group");
    let border = requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("fill") == Some("none")
                && node.ancestors().any(|ancestor| {
                    ancestor.attribute("class") == Some("basic label-container outer-path")
                })
        })
        .expect("Neo Requirement border path");
    assert_eq!(border.attribute("stroke"), Some("#123456"));
    assert_eq!(border.attribute("style"), Some("stroke:#123456 !important"));
    let divider = requirement
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.ancestors().any(|ancestor| {
                    ancestor.has_tag_name("g") && ancestor.attribute("class") == Some("divider")
                })
        })
        .expect("Neo Requirement divider path");
    assert_eq!(divider.attribute("stroke"), Some("#123456"));
    assert_eq!(
        divider.attribute("style"),
        Some("stroke:#123456 !important")
    );
    assert!(
        rendered
            .svg()
            .contains(r#"[data-look="neo"].node path{stroke:"#),
        "the regression requires a competing Neo path rule"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_source_class_and_inline_strokes_outrank_typed_stroke_per_node() {
    let source = r#"requirementDiagram
  requirement bare {
    id: 1
    text: Typed owner
    risk: low
    verifymethod: analysis
  }
  requirement assigned {
    id: 2
    text: Class owner
    risk: medium
    verifymethod: test
  }
  element inline {
    type: simulation
    docref: inline-owner
  }
  classDef accent stroke:#00aa00
  class assigned,inline accent
  style inline stroke:#0000aa
"#;
    let theme = requirement_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid mixed Requirement stroke"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid mixed-owner Requirement SVG");

    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-bare"),
        "#123456"
    );
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-assigned"),
        "#00aa00"
    );
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-inline"),
        "#0000aa"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_source_owned_fill_does_not_hide_direct_stroke_evidence() {
    let source = r#"requirementDiagram
  requirement req1 {
    id: 1
    text: Property-local owner
    risk: low
    verifymethod: analysis
  }
  style req1 fill:#00aa00
"#;
    let theme = requirement_fill_and_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid suppressed Requirement fill"),
        CanvasPaint::solid("#123456").expect("valid direct Requirement stroke"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid property-local Requirement SVG");

    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#00aa00"
    );
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-req1"),
        "#123456"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_explicit_default_paint_reaches_terminal_receipt() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default()
                            .with_fill(
                                CanvasPaint::solid("#654321")
                                    .expect("valid explicit-Default Requirement fill"),
                            )
                            .with_stroke(
                                CanvasPaint::solid("#123456")
                                    .expect("valid explicit-Default Requirement stroke"),
                            ),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
            ),
        )
        .expect("compile explicit Default Requirement paint");
    let rendered = render_requirement_with_theme(requirement_source(), &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid explicit-Default Requirement SVG");
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#654321"
    );
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-req1"),
        "#123456"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_explicit_node_border_outranks_typed_stroke() {
    let theme = requirement_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid config-owned Requirement stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "nodeBorder": "#fedcba" }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid config-owned Requirement SVG");
    assert_eq!(
        terminal_stroke_for(&document, "requirement-theme-req1"),
        "#fedcba"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_source_class_and_inline_fills_outrank_typed_fill_per_node() {
    let source = r#"requirementDiagram
  requirement bare {
    id: 1
    text: Typed owner
    risk: low
    verifymethod: analysis
  }
  requirement assigned {
    id: 2
    text: Class owner
    risk: medium
    verifymethod: test
  }
  element inline {
    type: simulation
    docref: inline-owner
  }
  classDef accent fill:#00aa00
  class assigned,inline accent
  style inline fill:#0000aa
"#;
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid mixed Requirement fill"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid mixed-owner Requirement SVG");

    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-bare"),
        "#123456"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-assigned"),
        "#00aa00"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-inline"),
        "#0000aa"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_source_styles_reject_css_injection_tokens() {
    let source = r#"requirementDiagram
  requirement unsafe {
    id: 1
    text: Unsafe source style
    risk: low
    verifymethod: analysis
  }
  classDef unsafe fill:#123456;stroke:url(javascript:alert(1))
  class unsafe unsafe
  style unsafe stroke:#654321;fill:red
"#;
    let theme = requirement_fill_and_stroke_theme(
        CanvasPaint::solid("#abcdef").expect("valid Requirement fill"),
        CanvasPaint::solid("#fedcba").expect("valid Requirement stroke"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Requirement SVG");
    let requirement = document
        .descendants()
        .find(|node| node.attribute("id") == Some("requirement-theme-unsafe"))
        .expect("unsafe Requirement node");
    let emitted_styles = requirement
        .descendants()
        .filter_map(|node| node.attribute("style"))
        .collect::<Vec<_>>()
        .join(";");

    assert!(
        !emitted_styles.contains("url("),
        "unsafe URL reached SVG: {emitted_styles}"
    );
    assert!(!emitted_styles.contains("javascript:"));
    assert!(!emitted_styles.contains(";fill:red"));
}

#[test]
fn requirement_fill_is_not_applicable_when_every_node_has_a_source_owner() {
    let source = r#"requirementDiagram
  requirement assigned {
    id: 1
    text: Class owner
    risk: medium
    verifymethod: test
  }
  element inline {
    type: simulation
    docref: inline-owner
  }
  classDef accent fill:#00aa00
  class assigned accent
  style inline fill:#0000aa
"#;
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid suppressed Requirement fill"),
    );
    let rendered = render_requirement_with_theme(source, &theme);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid source-owned Requirement SVG");
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-assigned"),
        "#00aa00"
    );
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-inline"),
        "#0000aa"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_ordinal_palette_is_not_applicable_when_typed_fill_wins() {
    let theme = requirement_fill_with_ordinal_palette_theme(
        CanvasPaint::solid("#123456").expect("valid typed Requirement fill"),
    );
    let rendered = render_requirement_with_theme(requirement_source(), &theme);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn requirement_ordinal_palette_is_not_applicable_when_source_fill_wins() {
    let source = r#"requirementDiagram
  requirement assigned {
    id: 1
    text: Class owner
    risk: medium
    verifymethod: test
  }
  classDef accent fill:#00aa00
  class assigned accent
"#;
    let rendered = render_requirement_with_theme(source, &requirement_ordinal_palette_theme());
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn requirement_explicit_background_outranks_typed_fill() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid config-owned Requirement fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "requirementBackground": "#fedcba" }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid config-owned Requirement SVG");
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#fedcba"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn extended_dark_primary_color_derived_background_outranks_typed_fill() {
    let theme = requirement_fill_theme(
        CanvasPaint::solid("#123456").expect("valid config-derived Requirement fill"),
    );
    let source_owned = format!(
        "%%{{init: {{\"themeVariables\": {{\"primaryColor\": \"#fedcba\"}}}}}}%%\n{}",
        requirement_source()
    );
    let cases = [
        (
            "site-owned neo-dark",
            requirement_source().to_string(),
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "theme": "neo-dark",
                "themeVariables": { "primaryColor": "#fedcba" }
            }))),
            "#fedcba",
            0,
            1,
        ),
        (
            "source-owned redux-dark",
            source_owned,
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "theme": "redux-dark",
                "secure": []
            }))),
            "#123456",
            1,
            0,
        ),
    ];

    for (owner, source, engine, expected_fill, applied_count, not_applicable_count) in cases {
        let rendered = render_requirement_with_theme_and_engine(&source, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("valid {owner} Requirement SVG: {error}"));
        assert_eq!(
            terminal_fill_for(&document, "requirement-theme-req1"),
            expected_fill,
            "{owner} primaryColor must retain the terminal fill"
        );

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{owner}");
        assert_eq!(evidence.applied_count(), applied_count, "{owner}");
        assert_eq!(
            evidence.not_applicable_count(),
            not_applicable_count,
            "{owner}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    }
}

#[test]
fn requirement_border_color_array_owns_stroke_without_hiding_typed_fill() {
    let theme = requirement_fill_and_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid typed Requirement fill"),
        CanvasPaint::solid("#654321").expect("valid typed Requirement stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "borderColorArray": ["#fedcba"]
        }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid borderColorArray Requirement SVG");

    // borderColorArray only owns the stroke surface; the sibling fill remains typed-owned.
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#123456"
    );
    let css = requirement_stylesheet(rendered.svg());
    assert!(
        css.contains(
            r#"#requirement-theme [data-look="classic"][data-color-id="color-0"].node path{stroke:#fedcba;"#
        ),
        "borderColorArray must own the terminal stroke selector"
    );
    assert!(
        !css.contains(
            r#"#requirement-theme [data-look="classic"][data-color-id="color-0"].node path{stroke:#654321;"#
        ),
        "typed stroke must not overwrite the borderColorArray owner"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn requirement_color_arrays_own_fill_and_stroke_without_false_applied_evidence() {
    let theme = requirement_fill_and_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid typed Requirement fill"),
        CanvasPaint::solid("#654321").expect("valid typed Requirement stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "borderColorArray": ["#fedcba"],
            "bkgColorArray": ["#abcdef"]
        }
    })));
    let rendered = render_requirement_with_theme_and_engine(requirement_source(), &theme, engine);
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Requirement color-array SVG");

    // The paired Mermaid arrays own both terminal paints. The typed values must not be emitted
    // as inline winners, otherwise the CSS palette would be visually and evidentially bypassed.
    assert_eq!(
        terminal_fill_for(&document, "requirement-theme-req1"),
        "#ECECFF"
    );
    let css = requirement_stylesheet(rendered.svg());
    let selector =
        r#"#requirement-theme [data-look="classic"][data-color-id="color-0"].node path{"#;
    assert!(
        css.contains(&format!("{selector}stroke:#fedcba;fill:#abcdef;")),
        "paired arrays must own the Requirement stroke and fill selectors"
    );
    assert!(
        !css.contains("#123456") && !css.contains("#654321"),
        "typed paint must not be re-emitted as a competing stylesheet winner"
    );

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}
