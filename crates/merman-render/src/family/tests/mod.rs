use super::*;
use crate::RenderCapability;
use crate::diagram_theme::{
    BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, FontStack, GradientStop, LinearGradient, MermaidThemeCompatibility,
    OrdinalPalette, OrdinalSelector, PatternKind, PatternSpec, RootThemeEvaluation,
    RootThemeMechanismKey, RootThemeVerification, Specified, TextStylePatch, ThemeCapability,
    ThemeColorValue, ThemeGeometryPatch, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, TypographySpec,
};
#[cfg(feature = "layout-cytoscape")]
use std::sync::Arc;
use std::sync::Mutex;

use crate::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    TextMeasurementOperation, TextMeasurementPhase, TextMeasurementResultKind,
};
#[cfg(feature = "layout-cytoscape")]
use crate::environment::{
    MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use crate::svg::SvgPipelinePreset;
use crate::text::{TextMetrics, WrapMode};
use merman_core::__private::{
    ThemeCompatibilityPlan, ThemeFamilyCompatibilityOverlayBuilder, install_theme_compatibility,
    theme_parse_evidence,
};
use merman_core::{
    CustomJsonProvenance, CustomJsonRenderModel, Engine, MermaidConfig, ParseOptions,
};
use serde_json::{Value, json};

fn custom_semantic_parser(
    _code: &str,
    meta: &ParseMetadata,
    control: &merman_core::OperationControl,
) -> merman_core::OperationControlResult<merman_core::Result<Value>> {
    control.checkpoint()?;
    Ok(Ok(
        json!({ "type": meta.diagram_type, "owner": "semantic" }),
    ))
}

fn custom_render_parser(
    _code: &str,
    _meta: &ParseMetadata,
    control: &merman_core::OperationControl,
) -> merman_core::OperationControlResult<merman_core::Result<CustomJsonRenderModel>> {
    control.checkpoint()?;
    Ok(Ok(CustomJsonRenderModel::new(
        "custom-flowchart",
        json!({ "owner": "render" }),
    )))
}

fn session() -> RenderSession {
    crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap()
}

fn flowchart_node_theme(style: ThemeStylePatch) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::Node, style).for_family(DiagramFamilyId::FLOWCHART),
            )),
        )
        .expect("compile Flowchart Node theme")
}

fn class_relation_theme(
    style: ThemeStylePatch,
    variant: Option<crate::diagram_theme::ThemeVariant>,
) -> DiagramTheme {
    let mut rule = ThemeRule::new(ThemeTarget::Edge, style).for_family(DiagramFamilyId::CLASS);
    if let Some(variant) = variant {
        rule = rule.with_variant(variant);
    }
    class_rule_theme(rule)
}

fn class_rule_theme(rule: ThemeRule) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .expect("compile Class theme rule")
}

fn flowchart_edge_stroke_theme(family: DiagramFamilyId, paint: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke(paint),
                    )
                    .for_family(family),
                ),
            ),
        )
        .expect("compile Flowchart Edge stroke theme")
}

fn flowchart_node_stroke_width_theme(family: DiagramFamilyId, stroke_width: f32) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_stroke_width(stroke_width)
                            .expect("valid Flowchart stroke width"),
                    )
                    .for_family(family),
                ),
            ),
        )
        .expect("compile Flowchart Node stroke-width theme")
}

fn flowchart_node_stroke_dasharray_theme(
    family: DiagramFamilyId,
    stroke_dasharray: impl IntoIterator<Item = f32>,
) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_stroke_dasharray(stroke_dasharray)
                            .expect("valid Flowchart stroke dasharray"),
                    )
                    .for_family(family),
                ),
            ),
        )
        .expect("compile Flowchart Node stroke-dasharray theme")
}

fn flowchart_edge_stroke_dasharray_theme(
    family: DiagramFamilyId,
    stroke_dasharray: impl IntoIterator<Item = f32>,
) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default()
                            .with_stroke_dasharray(stroke_dasharray)
                            .expect("valid Flowchart Edge stroke dasharray"),
                    )
                    .for_family(family),
                ),
            ),
        )
        .expect("compile Flowchart Edge stroke-dasharray theme")
}

fn flowchart_node_radius_theme(family: DiagramFamilyId, radius: f32) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch {
                            geometry: ThemeGeometryPatch {
                                radius: Specified::Value(radius),
                            },
                            ..ThemeStylePatch::default()
                        },
                    )
                    .for_family(family),
                ),
            ),
        )
        .expect("compile Flowchart Node radius theme")
}

fn flowchart_node_label_font_stack_theme(family: DiagramFamilyId) -> DiagramTheme {
    flowchart_node_label_font_stack_theme_with_base(family, None)
}

fn flowchart_node_label_font_stack_theme_with_base(
    family: DiagramFamilyId,
    base_font_stack: Option<FontStack>,
) -> DiagramTheme {
    flowchart_node_label_typography_theme(family, base_font_stack, None, None)
}

fn flowchart_node_label_font_stack_and_size_theme_with_base(
    family: DiagramFamilyId,
    base_font_size_px: Option<f32>,
) -> DiagramTheme {
    flowchart_node_label_typography_theme(family, None, Some(26.0), base_font_size_px)
}

fn flowchart_node_label_font_size_theme_without_catalog(family: DiagramFamilyId) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch {
                            typography: TextStylePatch {
                                font_size_px: Specified::Value(26.0),
                                ..TextStylePatch::default()
                            },
                            ..ThemeStylePatch::default()
                        },
                    )
                    .for_family(family),
                ),
            ),
        )
        .expect("compile Flowchart NodeLabel font-size theme without a font catalog")
}

fn flowchart_node_label_typography_theme(
    family: DiagramFamilyId,
    base_font_stack: Option<FontStack>,
    node_label_font_size_px: Option<f32>,
    base_font_size_px: Option<f32>,
) -> DiagramTheme {
    let latin = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let cjk = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
    ));
    let mut styles = ThemeRuleSet::default().with_rule(
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
    if let Some(font_size_px) = node_label_font_size_px {
        styles = styles.with_rule(
            ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch {
                    typography: TextStylePatch {
                        font_size_px: Specified::Value(font_size_px),
                        ..TextStylePatch::default()
                    },
                    ..ThemeStylePatch::default()
                },
            )
            .for_family(family),
        );
    }
    let mut spec = DiagramThemeSpec::new().with_styles(styles).with_assets(
        crate::diagram_theme::ThemeAssets::default().with_font_catalog(
            crate::diagram_theme::FontCatalogSpec::new([
                crate::diagram_theme::FontAssetSpec::new("excalifont", latin),
                crate::diagram_theme::FontAssetSpec::new("xiaolai", cjk),
            ]),
        ),
    );
    if base_font_stack.is_some() || base_font_size_px.is_some() {
        let mut base = ThemeTextStyle::default();
        if let Some(base_font_stack) = base_font_stack {
            base = base.with_font_stack(base_font_stack);
        }
        if let Some(base_font_size_px) = base_font_size_px {
            base = base
                .with_font_size_px(base_font_size_px)
                .expect("valid base font size");
        }
        spec = spec.with_typography(TypographySpec::default().with_default(base));
    }
    DiagramThemeCompiler::new()
        .compile(spec)
        .expect("compile Flowchart NodeLabel font-stack theme")
}

fn flowchart_node_wrapper<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    node_id: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-et") == Some("node")
                && node.attribute("data-id") == Some(node_id)
        })
        .unwrap_or_else(|| panic!("Flowchart node wrapper for {node_id}"))
}

fn flowchart_node_shape_style(svg: &str, node_id: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    let wrapper = flowchart_node_wrapper(&document, node_id);
    wrapper
        .descendants()
        .find(|node| {
            node.is_element()
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "label-container")
                })
        })
        .and_then(|node| node.attribute("style"))
        .unwrap_or_else(|| panic!("Flowchart node shape style for {node_id}"))
        .to_string()
}

fn flowchart_edge_path_style(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .and_then(|node| node.attribute("style"))
        .expect("Flowchart edge path style")
        .to_string()
}

fn flowchart_edge_marker_id(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    let marker = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .and_then(|node| node.attribute("marker-end"))
        .expect("Flowchart edge marker-end");
    marker
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
        .expect("Flowchart marker-end URL")
        .to_string()
}

fn flowchart_marker_contains(svg: &str, marker_id: &str, needle: &str) -> bool {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    let marker = document
        .descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
        .unwrap_or_else(|| panic!("Flowchart marker {marker_id}"));
    marker
        .descendants()
        .filter(|node| node.is_element())
        .any(|node| {
            node.attributes()
                .any(|attribute| attribute.value().contains(needle))
        })
}

fn flowchart_node_shape_attribute(svg: &str, node_id: &str, attribute: &str) -> Option<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    let wrapper = flowchart_node_wrapper(&document, node_id);
    wrapper
        .descendants()
        .find(|node| {
            node.is_element()
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "label-container")
                })
        })
        .and_then(|node| node.attribute(attribute))
        .map(str::to_string)
}

fn flowchart_node_label_style(svg: &str, node_id: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    let wrapper = flowchart_node_wrapper(&document, node_id);
    wrapper
        .descendants()
        .find(|node| {
            node.is_element()
                && node
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "label"))
        })
        .and_then(|node| node.attribute("style"))
        .unwrap_or_else(|| panic!("Flowchart node label style for {node_id}"))
        .to_string()
}

fn family_report(
    evaluation: FamilyStyleEvaluation,
    required: Vec<FamilyThemeMechanismKey>,
    applied: Vec<FamilyThemeMechanismKey>,
    theme_residuals: Vec<FamilyThemeResidual>,
) -> FamilyStyleReport {
    FamilyStyleReport {
        family_id: DiagramFamilyId::STATE,
        evaluation,
        output_mutated: false,
        theme_required: required,
        theme_applied: applied,
        native_filter_receipt: None,
        theme_not_applicable: Vec::new(),
        theme_residuals,
        compatibility_residual_count: 0,
        mermaid_compatibility_residual_count: 0,
        residuals: Vec::new(),
    }
}

#[test]
fn family_theme_verification_state_matrix_is_fail_closed() {
    let key = FamilyThemeMechanismKey::Rule {
        index: 0,
        target: ThemeTarget::State,
    };
    let residual = FamilyThemeResidual {
        key: key.clone(),
        reason: FamilyThemeResidualReason::UnsupportedPaint,
    };

    assert_eq!(
        family_report(FamilyStyleEvaluation::NotApplicable, vec![], vec![], vec![]).verification(),
        FamilyStyleVerification::NotApplicable
    );
    assert_eq!(
        family_report(
            FamilyStyleEvaluation::Unadapted,
            vec![key.clone()],
            vec![],
            vec![residual.clone()],
        )
        .verification(),
        FamilyStyleVerification::Unadapted
    );
    assert_eq!(
        family_report(
            FamilyStyleEvaluation::Evaluated,
            vec![key.clone()],
            vec![key.clone()],
            vec![],
        )
        .verification(),
        FamilyStyleVerification::Verified
    );
    assert_eq!(
        family_report(
            FamilyStyleEvaluation::Evaluated,
            vec![key.clone()],
            vec![],
            vec![residual],
        )
        .verification(),
        FamilyStyleVerification::Unverified
    );
    assert_eq!(
        family_report(
            FamilyStyleEvaluation::Evaluated,
            vec![key.clone()],
            vec![],
            vec![],
        )
        .verification(),
        FamilyStyleVerification::Incomplete
    );
    assert_eq!(
        family_report(
            FamilyStyleEvaluation::Evaluated,
            vec![key.clone(), key.clone()],
            vec![key.clone()],
            vec![],
        )
        .verification(),
        FamilyStyleVerification::Incomplete
    );

    let error = family_report(FamilyStyleEvaluation::Evaluated, vec![key], vec![], vec![])
        .ensure_portable()
        .expect_err("strict portability must reject incomplete family evidence");
    assert_eq!(
        error.incomplete_family_theme(),
        Some((DiagramFamilyId::STATE, 1, 0))
    );

    let mut mermaid_compatibility =
        family_report(FamilyStyleEvaluation::NotApplicable, vec![], vec![], vec![]);
    mermaid_compatibility.mermaid_compatibility_residual_count = 1;
    assert_eq!(
        mermaid_compatibility.verification(),
        FamilyStyleVerification::Unverified
    );
    assert!(matches!(
        mermaid_compatibility.ensure_portable(),
        Err(Error::MermaidThemeCompatibility {
            family_id: DiagramFamilyId::STATE,
            residual_count: 1,
        })
    ));
}

#[test]
fn require_portable_rejects_explicit_mermaid_compatibility_in_the_low_level_api() {
    let compatibility = MermaidThemeCompatibility::default()
        .with_variable("primaryColor", "#ef4444")
        .expect("valid Mermaid compatibility value");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_mermaid_compatibility(compatibility))
        .expect("compile explicit Mermaid compatibility theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\nReady --> Done\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable session");

    let error = match prepare(parsed, &LayoutOptions::default(), session) {
        Ok(_) => {
            panic!("strict low-level rendering must reject explicit Mermaid compatibility")
        }
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::MermaidThemeCompatibility {
            family_id: DiagramFamilyId::STATE,
            residual_count: 1,
        }
    ));
}

#[test]
fn unknown_fallback_contributions_are_always_portability_residuals() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty typed theme");
    let mut overlay = ThemeFamilyCompatibilityOverlayBuilder::new("state", "test.compatibility.");
    overlay
        .try_push(
            "unknown-fallback",
            MermaidConfig::from_value(json!({
                "state": {"titleTopMargin": 77}
            })),
        )
        .expect("bounded fallback contribution");
    let overlay = overlay.finish();
    let plan = ThemeCompatibilityPlan::try_new(
        *theme.recipe_fingerprint().as_bytes(),
        MermaidConfig::empty_object(),
        move |family, _control| Ok((family == "state").then(|| overlay.clone())),
    )
    .expect("bounded compatibility plan");
    let parsed = install_theme_compatibility(Engine::new(), &plan)
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\nReady --> Done\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    assert_eq!(
        parsed.metadata().effective_config.as_value()["state"]["titleTopMargin"],
        json!(77)
    );
    assert_eq!(
        theme_parse_evidence(parsed.metadata()).fallback_contribution_count(),
        1
    );
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable session");

    let error = match prepare(parsed, &LayoutOptions::default(), session) {
        Ok(_) => panic!("unknown fallback config must remain an explicit residual"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::LegacyFamilyThemeCompatibility {
            family_id: DiagramFamilyId::STATE,
            residual_count: 1,
            ..
        }
    ));
}

struct RejectingTextLayoutBackend {
    identity: crate::text::TextLayoutBackendIdentity,
}

impl crate::text::TextLayoutBackend for RejectingTextLayoutBackend {
    fn identity(&self) -> &crate::text::TextLayoutBackendIdentity {
        &self.identity
    }

    fn capabilities(&self) -> crate::text::TextLayoutCapabilities {
        crate::text::TextLayoutCapabilities::native()
    }

    fn prepare_catalog(
        &self,
        _request: &crate::text::PrepareCatalogRequest,
    ) -> std::result::Result<crate::text::PreparedTextLayoutResponse, crate::text::TextLayoutError>
    {
        Err(crate::text::TextLayoutError::BackendRejected)
    }
}

#[test]
fn custom_catalog_preparation_failure_uses_the_native_catalog_fallback() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let catalog =
        crate::diagram_theme::FontCatalogSpec::new([crate::diagram_theme::FontAssetSpec::new(
            "excalifont",
            bytes,
        )])
        .compile(&crate::diagram_theme::ThemeResourcePolicy::interactive())
        .expect("fixture catalog should compile");
    let backend = RejectingTextLayoutBackend {
        identity: crate::text::TextLayoutBackendIdentity::new("test.rejecting", "v1")
            .expect("test backend identity"),
    };
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_font_catalog(catalog)
        .with_text_layout_backend(std::sync::Arc::new(backend))
        .begin_session()
        .expect("runtime session should still capture preparation evidence");
    assert_eq!(session.text_layout_error(), None);
    let session_report = session.report();
    let prepared_report = session_report
        .prepared_text_layout()
        .expect("native fallback should prepare the custom catalog");
    assert!(prepared_report.face_count() > 0);
    assert_eq!(prepared_report.failed_attempt_count(), 1);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
        .expect("parse info")
        .expect("info render model");
    prepare(parsed, &LayoutOptions::default(), session)
        .expect("native fallback should allow layout to continue");
}

#[test]
fn capability_plan_reports_the_authoritative_render_family() {
    let configured_swimlane = Engine::new().with_site_config(
        merman_core::MermaidConfig::from_value(json!({ "layout": "swimlane" })),
    );
    let cases = [
        (
            Engine::new(),
            "flowchart TD\nA --> B\n",
            DiagramFamilyId::FLOWCHART,
        ),
        (
            Engine::new(),
            "swimlane-beta LR\nA --> B\n",
            DiagramFamilyId::SWIMLANE,
        ),
        (
            configured_swimlane,
            "flowchart LR\nA --> B\n",
            DiagramFamilyId::SWIMLANE,
        ),
    ];

    for (engine, source, expected_family) in cases {
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("family fixture should produce a render model");
        let plan = plan_render(&parsed, &session()).unwrap();

        assert_eq!(plan.family_id(), expected_family, "{source}");
    }
}

#[test]
fn family_theme_plan_tracks_the_authoritative_family() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile test theme");
    let configured_swimlane = Engine::new().with_site_config(
        merman_core::MermaidConfig::from_value(json!({ "layout": "swimlane" })),
    );
    let cases = [
        (
            Engine::new(),
            "flowchart TD\nA --> B\n",
            DiagramFamilyId::FLOWCHART,
        ),
        (
            configured_swimlane,
            "flowchart LR\nA --> B\n",
            DiagramFamilyId::SWIMLANE,
        ),
    ];

    for (engine, source, expected_family) in cases {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("family fixture should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed render session");
        let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let resolved_theme = artifact
            .context
            .resolved_theme()
            .expect("themed artifact should retain a resolved family plan");

        assert_eq!(artifact.family_id(), expected_family, "{source}");
        assert_eq!(artifact.context.family_id(), expected_family, "{source}");
        assert_eq!(resolved_theme.family_id(), expected_family, "{source}");
        assert_eq!(
            artifact.context.session().theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint()),
            "{source}"
        );
    }
}

#[test]
fn family_render_report_freezes_after_pipeline_and_terminal_svg() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile test theme");
    let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(
        json!({ "layout": "swimlane" }),
    ));
    let parsed = theme
        .install_parse_compatibility(engine)
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin themed render session");
    let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed family SVG");
    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    assert_eq!(
        rendered.root_theme_report().evaluation(),
        RootThemeEvaluation::NotApplicable
    );
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::NotApplicable
    );
    assert_eq!(
        rendered.style_report().evaluation(),
        FamilyStyleEvaluation::NotApplicable
    );
    assert_eq!(
        rendered.session.theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );

    let rendered = rendered
        .apply_pipeline(&SvgPipeline::parity())
        .expect("apply themed family SVG pipeline");
    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    assert_eq!(
        rendered.session.theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );

    let finalized = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("finalize themed family SVG");
    assert_eq!(finalized.family_id(), DiagramFamilyId::SWIMLANE);
    assert!(!finalized.style_report().is_verified());
    assert_eq!(
        finalized.session.theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );

    let completion = finalized.into_completion();
    assert_eq!(
        completion.output().finalization_report().preset(),
        SvgPipelinePreset::ResvgSafe
    );
    assert_eq!(completion.report().family_id(), DiagramFamilyId::SWIMLANE);
    assert_eq!(
        completion.report().theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
    assert_eq!(
        completion
            .report()
            .session_report()
            .theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
    assert_eq!(
        completion.report().style_report(),
        &FamilyStyleReport {
            family_id: DiagramFamilyId::SWIMLANE,
            evaluation: FamilyStyleEvaluation::NotApplicable,
            output_mutated: false,
            theme_required: Vec::new(),
            theme_applied: Vec::new(),
            native_filter_receipt: None,
            theme_not_applicable: Vec::new(),
            theme_residuals: Vec::new(),
            compatibility_residual_count: 0,
            mermaid_compatibility_residual_count: 0,
            residuals: Vec::new(),
        }
    );
    assert_eq!(
        completion.report().root_theme_report().evaluation(),
        RootThemeEvaluation::NotApplicable
    );
}

#[test]
fn solid_root_layer_is_applied_by_terminal_svg_completion() {
    let layer = CanvasLayer::new(CanvasPaint::solid("#ef4444").expect("valid layer paint"))
        .with_opacity(0.5)
        .expect("valid layer opacity")
        .with_offset(4.0, -2.0)
        .expect("valid layer offset")
        .with_blend_mode(BlendMode::Multiply);
    let canvas = CanvasSpec::default()
        .with_layer(layer)
        .expect("bounded canvas layer");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(canvas))
        .expect("compile layered canvas theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin best-effort themed session");
    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("root theme should not stop layout")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed SVG");

    assert_eq!(
        rendered.root_theme_report().evaluation(),
        RootThemeEvaluation::Evaluated
    );
    assert_eq!(
        rendered.root_theme_report().verification(),
        RootThemeVerification::Verified
    );
    assert!(
        rendered
            .root_theme_report()
            .applied_mechanisms()
            .contains(&RootThemeMechanismKey::CanvasLayer { index: 0 })
    );
    assert!(rendered.root_theme_report().residuals().is_empty());
    assert!(rendered.svg().contains(
        r#"data-merman-theme-canvas-layer="0" aria-hidden="true" pointer-events="none""#
    ));
    assert!(rendered.svg().contains(r#"opacity="0.5""#));
    assert!(rendered.svg().contains(r#"transform="translate(4 -2)""#));
    assert!(
        rendered
            .svg()
            .contains(r#"style="mix-blend-mode:multiply""#)
    );
    assert!(rendered.svg().contains(r##"fill="#ef4444""##));

    let completion = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("finalize themed SVG")
        .into_completion();
    let root_report = completion.report().root_theme_report();
    assert!(root_report.residuals().is_empty());
    assert_eq!(root_report.verification(), RootThemeVerification::Verified);
    assert!(root_report.coverage_complete());
}

#[test]
fn solid_root_base_is_applied_by_terminal_svg_completion() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_canvas(CanvasSpec::solid("#111827").expect("valid canvas base")),
        )
        .expect("compile solid canvas theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin themed session");
    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("solid canvas should not stop layout")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed SVG");

    assert_eq!(
        rendered.root_theme_report().verification(),
        RootThemeVerification::Verified
    );
    assert!(
        rendered
            .root_theme_report()
            .applied_mechanisms()
            .contains(&RootThemeMechanismKey::CanvasBase)
    );
    assert!(
        rendered
            .svg()
            .contains(r#"class="merman-theme-canvas-base" data-merman-theme-canvas="base""#)
    );
    assert!(rendered.svg().contains(r##"fill="#111827""##));
}

#[test]
fn explicit_transparent_root_base_clears_the_mermaid_white_background() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(CanvasSpec::transparent()))
        .expect("compile transparent canvas theme");
    assert!(
        theme
            .report()
            .requires_capability(ThemeCapability::TransparentPaint)
    );
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin transparent themed session");
    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("transparent canvas should not stop layout")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render transparent themed SVG");

    assert_eq!(
        rendered.root_theme_report().verification(),
        RootThemeVerification::Verified
    );
    assert!(rendered.svg().contains("background-color: transparent;"));
    assert!(!rendered.svg().contains("background-color: white;"));
    assert!(
        rendered
            .svg()
            .contains(r#"data-merman-theme-canvas="base""#)
    );
    assert!(rendered.svg().contains(r#"fill="none""#));
}

#[test]
fn root_theme_evidence_is_invalidated_by_an_untrusted_svg_postprocessor() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_canvas(CanvasSpec::solid("#111827").expect("valid canvas paint")),
        )
        .expect("compile root canvas theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin themed render session");
    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed family")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed SVG")
        .apply_pipeline(
            &SvgPipeline::parity()
                .with_postprocessor(crate::svg::RootBackgroundPostprocessor::new("white")),
        )
        .expect("best-effort output may retain a downgraded report");

    assert_eq!(
        rendered.root_theme_report().verification(),
        RootThemeVerification::Unverified
    );
    assert_eq!(
        rendered.root_theme_report().residuals()[0].reason(),
        crate::diagram_theme::RootThemeResidualReason::OutputMutation
    );
}

#[test]
fn strict_root_theme_is_rechecked_after_svg_postprocessing() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_canvas(CanvasSpec::solid("#111827").expect("valid canvas paint")),
        )
        .expect("compile root canvas theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict themed render session");
    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed family")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("initial root consumer proves the canvas");
    let error = match rendered.apply_pipeline(
        &SvgPipeline::parity()
            .with_postprocessor(crate::svg::RootBackgroundPostprocessor::new("white")),
    ) {
        Ok(_) => panic!("strict portability must be rechecked after postprocessing"),
        Err(error) => error,
    };

    assert!(error.rejected_root_theme().is_some(), "{error}");
}

#[test]
fn family_theme_evidence_is_invalidated_by_untrusted_svg_postprocessing() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let render = |portability| {
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA[Alpha]\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(portability)
            .begin_session_with_theme(&theme)
            .expect("begin themed render session");
        prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare themed Flowchart")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("initial Node consumer proves typed paint")
    };
    let pipeline = || {
        SvgPipeline::parity()
            .with_postprocessor(crate::svg::RootBackgroundPostprocessor::new("white"))
    };

    let rendered = render(ThemePortabilityRequirement::BestEffort)
        .apply_pipeline(&pipeline())
        .expect("best-effort output retains downgraded family evidence");
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Unverified
    );
    assert!(rendered.style_report().output_mutated());
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            },
            reason: FamilyThemeResidualReason::OutputMutation,
        }]
    );

    let apply_error =
        match render(ThemePortabilityRequirement::RequirePortable).apply_pipeline(&pipeline()) {
            Ok(_) => panic!("strict draft output must reject invalidated family evidence"),
            Err(error) => error,
        };
    assert!(matches!(
        apply_error,
        Error::UnverifiedFamilyOutputMutation {
            family_id: DiagramFamilyId::FLOWCHART
        }
    ));

    let finalize_error = match render(ThemePortabilityRequirement::RequirePortable)
        .finalize_resvg(&pipeline().into_resvg_safe())
    {
        Ok(_) => panic!("strict finalized output must reject invalidated family evidence"),
        Err(error) => error,
    };
    assert!(matches!(
        finalize_error,
        Error::UnverifiedFamilyOutputMutation {
            family_id: DiagramFamilyId::FLOWCHART
        }
    ));

    let standalone_error = match render(ThemePortabilityRequirement::RequirePortable)
        .finalize_standalone(Some(&pipeline()))
    {
        Ok(_) => panic!("strict standalone output must reject invalidated family evidence"),
        Err(error) => error,
    };
    assert!(matches!(
        standalone_error,
        Error::UnverifiedFamilyOutputMutation {
            family_id: DiagramFamilyId::FLOWCHART
        }
    ));
}

#[test]
fn unsupported_root_pattern_residual_survives_terminal_svg_completion() {
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#22d3ee").unwrap(),
    )
    .unwrap();
    let canvas = CanvasSpec::default()
        .with_layer(CanvasLayer::new(CanvasPaint::Pattern(pattern)))
        .expect("bounded pattern canvas layer");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(canvas))
        .expect("compile layered canvas theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin best-effort themed session");
    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("best-effort root residual should not stop layout")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render best-effort SVG");

    let report = rendered.root_theme_report();
    assert_eq!(report.evaluation(), RootThemeEvaluation::Evaluated);
    assert_eq!(report.verification(), RootThemeVerification::Unverified);
    assert_eq!(report.residuals().len(), 1);
    assert_eq!(
        report.residuals()[0].key(),
        &RootThemeMechanismKey::CanvasLayer { index: 0 }
    );
    assert!(!rendered.svg().contains("data-merman-theme-canvas-layer"));
}

#[test]
fn root_theme_evidence_preserves_distinct_layers_with_equal_capabilities() {
    let canvas = CanvasSpec::default()
        .with_layer(CanvasLayer::new(
            CanvasPaint::solid("#ef4444").expect("valid first layer paint"),
        ))
        .expect("bounded first canvas layer")
        .with_layer(CanvasLayer::new(
            CanvasPaint::solid("#2563eb").expect("valid second layer paint"),
        ))
        .expect("bounded second canvas layer");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(canvas))
        .expect("compile layered canvas theme");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin best-effort themed session");
    let plan = RootThemePlan::from_theme(session.theme());
    let mut application = plan.begin_svg_application();
    assert!(application.mark_applied(
        &RootThemeMechanismKey::CanvasLayer { index: 0 },
        [ThemeCapability::LayeredCanvas, ThemeCapability::SolidPaint],
    ));
    assert!(application.mark_applied(
        &RootThemeMechanismKey::CanvasLayer { index: 1 },
        [ThemeCapability::LayeredCanvas, ThemeCapability::SolidPaint],
    ));
    let report = application.finish();

    assert_eq!(
        report.required_mechanisms(),
        &[
            RootThemeMechanismKey::CanvasLayer { index: 0 },
            RootThemeMechanismKey::CanvasLayer { index: 1 },
        ]
    );
    assert!(report.residuals().is_empty());
    assert_eq!(
        report.applied_mechanisms(),
        &[
            RootThemeMechanismKey::CanvasLayer { index: 0 },
            RootThemeMechanismKey::CanvasLayer { index: 1 },
        ]
    );
    assert!(report.coverage_complete());
}

#[test]
fn root_layer_evidence_keeps_opacity_and_offset_capabilities_on_the_layer_key() {
    let layer = CanvasLayer::new(CanvasPaint::solid("#ef4444").expect("valid layer paint"))
        .with_opacity(0.5)
        .expect("valid layer opacity")
        .with_offset(4.0, -2.0)
        .expect("valid layer offset");
    let canvas = CanvasSpec::default()
        .with_layer(layer)
        .expect("bounded canvas layer");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(canvas))
        .expect("compile layered canvas theme");
    let report = RootThemePlan::from_theme(Some(&theme))
        .begin_svg_application()
        .finish();
    let mechanism = &report.mechanisms()[0];

    assert!(
        mechanism
            .required_capabilities()
            .any(|capability| capability == ThemeCapability::Opacity)
    );
    assert!(
        mechanism
            .required_capabilities()
            .any(|capability| capability == ThemeCapability::CanvasLayerPlacement)
    );
    assert!(
        mechanism
            .residual_capabilities()
            .any(|capability| capability == ThemeCapability::CanvasLayerPlacement)
    );
}

#[test]
fn require_portable_accepts_a_root_layer_proved_by_the_svg_consumer() {
    let canvas = CanvasSpec::transparent()
        .with_layer(CanvasLayer::new(
            CanvasPaint::solid("#ef4444").expect("valid layer paint"),
        ))
        .expect("bounded canvas layer");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(canvas))
        .expect("compile layered canvas theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("strict root theme should reach the real SVG consumer")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("proved root layer should satisfy strict portability");

    assert_eq!(
        rendered.root_theme_report().verification(),
        RootThemeVerification::Verified
    );
}

#[test]
fn require_portable_rejects_an_unsupported_root_pattern_after_svg_consumption() {
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#22d3ee").unwrap(),
    )
    .unwrap();
    let canvas = CanvasSpec::transparent()
        .with_layer(CanvasLayer::new(CanvasPaint::Pattern(pattern)))
        .expect("bounded pattern layer");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(canvas))
        .expect("compile pattern theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable session");
    let artifact = prepare(parsed, &LayoutOptions::default(), session)
        .expect("strict root theme should reach the real SVG consumer");

    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("strict portability must reject an unsupported root pattern"),
        Err(error) => error,
    };
    let (verification, residual_count) = error
        .rejected_root_theme()
        .expect("evaluated root theme rejection");
    assert_eq!(verification, RootThemeVerification::Unverified);
    assert_eq!(residual_count, 1);
}

#[test]
fn require_portable_accepts_flowchart_typed_node_paint_after_svg_emission() {
    let fill = CanvasPaint::solid("#ef4444").expect("valid node fill");
    let stroke = CanvasPaint::solid("#2563eb").expect("valid node stroke");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(fill)
                            .with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        )
        .expect("compile strict Flowchart theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable render session");

    let artifact = prepare(parsed, &LayoutOptions::default(), session)
        .expect("typed Flowchart paint must reach SVG emission");
    let (preparation_evidence, source_residuals) = artifact
        .family
        .flowchart_theme_evidence(artifact.context.resolved_theme())
        .expect("Flowchart artifact evidence");
    assert!(source_residuals.is_empty());
    assert!(preparation_evidence.applied().is_empty());

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("emitted typed Flowchart paint must satisfy strict portability");

    assert!(rendered.svg().contains("fill:#ef4444 !important"));
    assert!(rendered.svg().contains("stroke:#2563eb !important"));
    let report = rendered.style_report();
    assert_eq!(report.evaluation(), FamilyStyleEvaluation::Evaluated);
    assert_eq!(report.verification(), FamilyStyleVerification::Verified);
    assert_eq!(report.compatibility_residual_count(), 0);
    assert_eq!(
        report.theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
}

#[test]
fn require_portable_accepts_swimlane_typed_node_paint_after_svg_emission() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::Transparent)
                            .with_stroke(CanvasPaint::Transparent),
                    )
                    .for_family(DiagramFamilyId::SWIMLANE),
                ),
            ),
        )
        .expect("compile strict Swimlane theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA --> B\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Swimlane source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable render session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("typed Swimlane paint must reach SVG emission")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("emitted typed Swimlane paint must satisfy strict portability");

    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    assert!(
        rendered
            .svg()
            .contains("fill:none !important;stroke:none !important")
    );
    let report = rendered.style_report();
    assert_eq!(report.evaluation(), FamilyStyleEvaluation::Evaluated);
    assert_eq!(report.verification(), FamilyStyleVerification::Verified);
    assert_eq!(report.compatibility_residual_count(), 0);
    assert_eq!(
        report.theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
}

#[test]
fn flowchart_source_paint_override_does_not_claim_typed_application() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap())
                            .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        )
        .expect("compile Flowchart source precedence theme");
    let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nclassDef source fill:#22c55e,stroke:#111827\nA[Alpha]:::source\nB[Beta]\nstyle B fill:#f59e0b,stroke:#334155\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable render session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("source-overridden typed paint remains evaluable")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("source-overridden typed paint is not a portability residual");

    assert!(rendered.svg().contains("fill:#22c55e !important"));
    assert!(rendered.svg().contains("stroke:#111827 !important"));
    assert!(rendered.svg().contains("fill:#f59e0b !important"));
    assert!(rendered.svg().contains("stroke:#334155 !important"));
    assert!(!rendered.svg().contains("fill:#ef4444 !important"));
    assert!(!rendered.svg().contains("stroke:#2563eb !important"));
    let report = rendered.style_report();
    assert_eq!(report.verification(), FamilyStyleVerification::Verified);
    assert!(report.theme_applied_mechanisms().is_empty());
    assert_eq!(
        report.theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
}

#[test]
fn flowchart_single_source_paint_override_keeps_other_typed_channel_applied() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap())
                            .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        )
        .expect("compile Flowchart source precedence theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "flowchart LR\nstyle A fill:#22c55e\nA[Alpha]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("single source override remains evaluable")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("the surviving typed stroke should satisfy strict portability");

    assert!(rendered.svg().contains("fill:#22c55e !important"));
    assert!(rendered.svg().contains("stroke:#2563eb !important"));
    assert!(!rendered.svg().contains("fill:#ef4444 !important"));
    assert!(rendered.style_report().theme_applied_mechanisms().contains(
        &FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }
    ));
}

#[test]
fn flowchart_source_paint_identity_and_value_admission_control_precedence() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let render = |source: &str| {
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");
        prepare(parsed, &LayoutOptions::default(), session)
            .expect("source paint precedence is evaluated during SVG emission")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("admitted source precedence must remain portable")
    };

    let overridden = render("flowchart LR\nstyle A FILL:#22c55e\nA[Alpha]\n");
    assert!(overridden.svg().contains("FILL:#22c55e !important"));
    assert!(!overridden.svg().contains("fill:#ef4444 !important"));
    assert!(
        overridden
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        overridden.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );

    let dynamic_source = "flowchart LR\nstyle A --paint:#22c55e,fill:var(--paint)\nA[Alpha]\n";
    let parse_dynamic = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(dynamic_source, ParseOptions::strict())
            .unwrap()
            .expect("dynamic source paint should produce a render model")
    };
    let dynamic = prepare(
        parse_dynamic(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort session"),
    )
    .expect("prepare dynamic source paint")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output must preserve dynamic source paint");
    assert!(dynamic.svg().contains("fill:var(--paint) !important"));
    assert!(!dynamic.svg().contains("fill:#ef4444 !important"));
    assert!(dynamic.style_report().theme_residuals().is_empty());
    assert_eq!(dynamic.style_report().residuals().len(), 1);
    assert_eq!(dynamic.style_report().residuals()[0].owner_id(), "A");
    assert_eq!(
        dynamic.style_report().residuals()[0].property(),
        Some("fill")
    );
    assert_eq!(
        dynamic.style_report().residuals()[0].reason(),
        FamilyStyleResidualReason::InvalidValue
    );

    let strict_session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict session");
    let artifact = prepare(parse_dynamic(), &LayoutOptions::default(), strict_session)
        .expect("dynamic source verification waits for SVG emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("unverified dynamic source paint must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style(),
        Some((
            DiagramFamilyId::FLOWCHART,
            dynamic.style_report().residuals().len(),
        ))
    );
}

#[test]
fn flowchart_explicit_mermaid_config_owns_typed_node_paint_precedence() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );

    for explicit_fill in ["#22c55e", "#ef4444"] {
        let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
            MermaidConfig::from_value(json!({
                "themeVariables": {"mainBkg": explicit_fill}
            })),
        ));
        let parsed = engine
            .parse_diagram_for_render_model_sync("flowchart LR\nA[Alpha]\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare explicit Mermaid config")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("explicit Mermaid config should supersede typed Node paint");

        assert!(rendered.svg().contains(explicit_fill), "{}", rendered.svg());
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn require_portable_accepts_flowchart_and_swimlane_node_stroke_width_after_emission() {
    for (family, source) in [
        (DiagramFamilyId::FLOWCHART, "flowchart LR\nA[Alpha]\n"),
        (
            DiagramFamilyId::SWIMLANE,
            "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\n",
        ),
    ] {
        let theme = flowchart_node_stroke_width_theme(family, 2.5);
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict stroke-width session"),
        )
        .expect("prepare typed Node stroke width")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("classic Process writer should prove typed Node stroke width");

        assert_eq!(rendered.family_id(), family);
        assert!(
            flowchart_node_shape_style(rendered.svg(), "A")
                .contains("stroke-width:2.5px !important")
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(rendered.style_report().compatibility_residual_count(), 0);
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn flowchart_source_and_explicit_config_own_node_stroke_width_precedence() {
    let theme = flowchart_node_stroke_width_theme(DiagramFamilyId::FLOWCHART, 2.5);
    let render = |engine: Engine, source: &str| {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict precedence session"),
        )
        .expect("prepare Flowchart stroke-width precedence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("explicit Mermaid owner should supersede typed Node stroke width")
    };

    let source = render(
        Engine::new(),
        "flowchart LR\nstyle A stroke-width:4px\nA[Alpha]\n",
    );
    assert!(flowchart_node_shape_style(source.svg(), "A").contains("stroke-width:4px !important"));
    assert!(!source.svg().contains("stroke-width:2.5px !important"));

    let configured = render(
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": {"strokeWidth": 4}
        }))),
        "flowchart LR\nA[Alpha]\n",
    );
    assert!(configured.svg().contains("stroke-width:4px;"));
    assert!(!configured.svg().contains("stroke-width:2.5px !important"));

    for rendered in [&source, &configured] {
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn unverified_flowchart_node_surfaces_keep_typed_stroke_width_fail_closed() {
    let theme = flowchart_node_stroke_width_theme(DiagramFamilyId::FLOWCHART, 2.5);
    let cases = [
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
A[Alpha]
"#,
        "flowchart LR\nA@{ shape: choice }\n",
    ];

    for source in cases {
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort stroke-width session"),
        )
        .expect("prepare unverified stroke-width surface")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should retain geometry evidence");
        assert_eq!(rendered.style_report().theme_residuals().len(), 1);
        assert_eq!(
            rendered.style_report().theme_residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict stroke-width session"),
        )
        .expect("writer support is decided during SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unverified Node stroke width must remain fail-closed"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::FLOWCHART, 1))
        );
    }
}

#[test]
fn require_portable_accepts_flowchart_and_swimlane_node_stroke_dasharray_after_emission() {
    for (family, source) in [
        (DiagramFamilyId::FLOWCHART, "flowchart LR\nA[Alpha]\n"),
        (
            DiagramFamilyId::SWIMLANE,
            "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\n",
        ),
    ] {
        let theme = flowchart_node_stroke_dasharray_theme(family, [4.0, 2.0]);
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict stroke-dasharray session"),
        )
        .expect("prepare typed Node stroke dasharray")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("classic Process writer should prove typed Node stroke dasharray");

        assert_eq!(rendered.family_id(), family);
        assert!(
            flowchart_node_shape_style(rendered.svg(), "A")
                .contains("stroke-dasharray:4 2 !important")
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn flowchart_source_owns_node_stroke_dasharray_precedence() {
    let theme = flowchart_node_stroke_dasharray_theme(DiagramFamilyId::FLOWCHART, [4.0, 2.0]);
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "flowchart LR\nstyle A stroke-dasharray:8 3\nA[Alpha]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict stroke-dasharray precedence session"),
    )
    .expect("prepare Flowchart stroke-dasharray precedence")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("source style should supersede typed Node stroke dasharray");

    let shape_style = flowchart_node_shape_style(rendered.svg(), "A");
    assert!(shape_style.contains("stroke-dasharray:8 3 !important"));
    assert!(!shape_style.contains("stroke-dasharray:4 2 !important"));
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn unverified_flowchart_node_surfaces_keep_typed_stroke_dasharray_fail_closed() {
    let theme = flowchart_node_stroke_dasharray_theme(DiagramFamilyId::FLOWCHART, [4.0, 2.0]);
    let cases = [
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
A[Alpha]
"#,
        "flowchart LR\nA@{ shape: choice }\n",
    ];

    for source in cases {
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort stroke-dasharray session"),
        )
        .expect("prepare unverified stroke-dasharray surface")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should retain dash evidence");
        assert_eq!(rendered.style_report().theme_residuals().len(), 1);
        assert_eq!(
            rendered.style_report().theme_residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict stroke-dasharray session"),
        )
        .expect("writer support is decided during SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unverified Node stroke dasharray must remain fail-closed"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::FLOWCHART, 1))
        );
    }
}

#[test]
fn require_portable_accepts_flowchart_and_swimlane_node_radius_after_emission() {
    for (family, source) in [
        (DiagramFamilyId::FLOWCHART, "flowchart LR\nA[Alpha]\n"),
        (
            DiagramFamilyId::SWIMLANE,
            "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\n",
        ),
    ] {
        let theme = flowchart_node_radius_theme(family, 8.0);
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict radius session"),
        )
        .expect("prepare typed Node radius")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("classic Process writer should prove typed Node radius");

        assert_eq!(rendered.family_id(), family);
        assert_eq!(
            flowchart_node_shape_attribute(rendered.svg(), "A", "rx").as_deref(),
            Some("8")
        );
        assert_eq!(
            flowchart_node_shape_attribute(rendered.svg(), "A", "ry").as_deref(),
            Some("8")
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn flowchart_source_and_explicit_config_own_node_radius_precedence() {
    let theme = flowchart_node_radius_theme(DiagramFamilyId::FLOWCHART, 8.0);
    let render = |engine: Engine, source: &str| {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict radius precedence session"),
        )
        .expect("prepare Flowchart radius precedence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("explicit Mermaid owner should supersede typed Node radius")
    };

    let source = render(
        Engine::new(),
        "flowchart LR\nstyle A rx:4px,ry:6px\nA[Alpha]\n",
    );
    let source_style = flowchart_node_shape_style(source.svg(), "A");
    assert!(source_style.contains("rx:4px !important"));
    assert!(source_style.contains("ry:6px !important"));
    assert_eq!(
        flowchart_node_shape_attribute(source.svg(), "A", "rx"),
        None
    );
    assert_eq!(
        flowchart_node_shape_attribute(source.svg(), "A", "ry"),
        None
    );

    let configured = render(
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "look": "neo",
            "themeVariables": {"radius": 4}
        }))),
        "flowchart LR\nA[Alpha]\n",
    );
    assert_eq!(
        flowchart_node_shape_attribute(configured.svg(), "A", "rx").as_deref(),
        Some("4")
    );
    assert_eq!(
        flowchart_node_shape_attribute(configured.svg(), "A", "ry").as_deref(),
        Some("4")
    );

    for rendered in [&source, &configured] {
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn unverified_flowchart_node_surfaces_keep_typed_radius_fail_closed() {
    let theme = flowchart_node_radius_theme(DiagramFamilyId::FLOWCHART, 8.0);
    let cases = [
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
A[Alpha]
"#,
        "flowchart LR\nA@{ shape: choice }\n",
    ];

    for source in cases {
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort radius session"),
        )
        .expect("prepare unverified radius surface")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should retain geometry evidence");
        assert_eq!(rendered.style_report().theme_residuals().len(), 1);
        assert_eq!(
            rendered.style_report().theme_residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict radius session"),
        )
        .expect("writer support is decided during SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unverified Node radius must remain fail-closed"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::FLOWCHART, 1))
        );
    }
}

#[test]
fn classic_flowchart_without_diagram_theme_does_not_emit_radius_attributes() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync("flowchart LR\nA[Alpha]\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let rendered = prepare(parsed, &LayoutOptions::default(), session())
        .expect("prepare default Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render default Flowchart");

    assert_eq!(
        flowchart_node_shape_attribute(rendered.svg(), "A", "rx"),
        None
    );
    assert_eq!(
        flowchart_node_shape_attribute(rendered.svg(), "A", "ry"),
        None
    );
}

#[test]
fn require_portable_accepts_flowchart_and_swimlane_node_label_typography_after_emission() {
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
A[Alpha]
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
flowchart TD
A[Alpha]
"#,
        ),
    ] {
        let theme = flowchart_node_label_font_stack_and_size_theme_with_base(family, None);
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict NodeLabel typography session"),
        )
        .expect("prepare typed NodeLabel typography")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("prepared SVG writer should prove typed NodeLabel typography");

        assert_eq!(rendered.family_id(), family);
        let label_style = flowchart_node_label_style(rendered.svg(), "A");
        assert!(
            label_style.contains("font-family:\"Excalifont\" !important"),
            "{label_style}"
        );
        assert!(
            label_style.contains("font-size:26px !important"),
            "{label_style}"
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[
                FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                },
                FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::NodeLabel,
                },
            ]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn typed_node_label_font_stack_outranks_legacy_base_typography() {
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
A[Alpha]
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
flowchart TD
A[Alpha]
"#,
        ),
    ] {
        let theme = flowchart_node_label_font_stack_theme_with_base(
            family,
            Some(FontStack::single("Xiaolai SC").expect("valid base font stack")),
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin mixed typography session"),
        )
        .expect("typed NodeLabel rule should outrank legacy base typography")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render mixed typography Flowchart");

        let label_style = flowchart_node_label_style(rendered.svg(), "A");
        assert!(
            label_style.contains("font-family:\"Excalifont\" !important"),
            "{label_style}"
        );
        assert!(!label_style.contains("Xiaolai"), "{label_style}");
        assert!(rendered.style_report().theme_applied_mechanisms().contains(
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            }
        ));
        assert!(
            !rendered
                .style_report()
                .theme_not_applicable_mechanisms()
                .contains(&FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                })
        );
    }
}

#[test]
fn typed_node_label_font_size_outranks_legacy_base_typography() {
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
A[Alpha]
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
flowchart TD
A[Alpha]
"#,
        ),
    ] {
        let theme = flowchart_node_label_font_stack_and_size_theme_with_base(family, Some(19.0));
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin mixed font-size session"),
        )
        .expect("typed NodeLabel font size should outrank legacy base typography")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render mixed font-size Flowchart");

        let label_style = flowchart_node_label_style(rendered.svg(), "A");
        assert!(
            label_style.contains("font-size:26px !important"),
            "{label_style}"
        );
        assert!(!label_style.contains("font-size:19px"), "{label_style}");
        assert!(rendered.style_report().theme_applied_mechanisms().contains(
            &FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::NodeLabel,
            }
        ));
    }
}

#[test]
fn flowchart_source_and_explicit_config_own_node_label_font_stack_precedence() {
    let theme = flowchart_node_label_font_stack_theme(DiagramFamilyId::FLOWCHART);
    let render = |engine: Engine, source: &str| {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict NodeLabel font-stack precedence session"),
        )
        .expect("prepare Flowchart NodeLabel font-stack precedence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("source or Mermaid config should supersede typed NodeLabel font stack")
    };

    let source = render(
        Engine::new(),
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
classDef local font-family:Xiaolai SC
A[测试]:::local
"#,
    );
    assert!(
        flowchart_node_label_style(source.svg(), "A")
            .contains("font-family:\"Xiaolai SC\" !important")
    );

    let configured = render(
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "htmlLabels": false,
            "flowchart": {"htmlLabels": false},
            "themeVariables": {"fontFamily": "Xiaolai SC"}
        }))),
        "flowchart LR\nA[测试]\n",
    );
    assert!(
        flowchart_node_label_style(configured.svg(), "A")
            .contains("font-family:\"Xiaolai SC\" !important")
    );

    for rendered in [&source, &configured] {
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn flowchart_source_and_explicit_config_own_only_node_label_font_size_precedence() {
    let theme =
        flowchart_node_label_font_stack_and_size_theme_with_base(DiagramFamilyId::FLOWCHART, None);
    let render = |engine: Engine, source: &str| {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict NodeLabel font-size precedence session"),
        )
        .expect("prepare Flowchart NodeLabel font-size precedence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("source or Mermaid config should supersede typed NodeLabel font size")
    };

    let source = render(
        Engine::new(),
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
classDef local font-size:22px
A[Alpha]:::local
"#,
    );
    assert!(flowchart_node_label_style(source.svg(), "A").contains("font-size:22px !important"));

    let configured = render(
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "htmlLabels": false,
            "flowchart": {"htmlLabels": false},
            "themeVariables": {"fontSize": "22px"}
        }))),
        "flowchart LR\nA[Alpha]\n",
    );
    assert!(
        flowchart_node_label_style(configured.svg(), "A").contains("font-size:22px !important")
    );

    for rendered in [&source, &configured] {
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            }]
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::NodeLabel,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn flowchart_node_label_typography_rejects_unprepared_label_modes() {
    let theme =
        flowchart_node_label_font_stack_and_size_theme_with_base(DiagramFamilyId::FLOWCHART, None);
    let cases = [
        "flowchart LR\nA[Alpha]\n",
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A["`**Alpha**`"]
"#,
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: icon, label: "Plain" }
style A font-family:Excalifont
"#,
    ];

    for source in cases {
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let result = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict unprepared NodeLabel session"),
        );

        assert!(matches!(
            result,
            Err(Error::TextLayout(
                crate::text::TextLayoutFailure::UnsupportedLabelMode
            ))
        ));
    }
}

#[test]
fn flowchart_node_label_font_size_without_prepared_text_is_residual() {
    let theme = flowchart_node_label_font_size_theme_without_catalog(DiagramFamilyId::FLOWCHART);
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A[Alpha]
"#;
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort NodeLabel font-size session"),
    )
    .expect("prepare best-effort NodeLabel font-size Flowchart")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render best-effort NodeLabel font-size SVG");

    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Unverified
    );
    assert_eq!(rendered.style_report().theme_residuals().len(), 1);
    assert_eq!(
        rendered.style_report().theme_residuals()[0].key(),
        &FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::NodeLabel,
        }
    );
    assert_eq!(
        rendered.style_report().theme_residuals()[0].reason(),
        FamilyThemeResidualReason::UnsupportedTypography
    );

    let strict_result = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict NodeLabel font-size session"),
    )
    .expect("prepare strict NodeLabel font-size Flowchart")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
    let strict_error = match strict_result {
        Ok(_) => panic!("strict output must reject unproven NodeLabel font size"),
        Err(error) => error,
    };
    assert!(matches!(
        strict_error,
        Error::UnverifiedFamilyTheme {
            family_id: DiagramFamilyId::FLOWCHART,
            residual_count: 1,
        }
    ));
}

#[test]
fn flowchart_node_without_label_makes_typed_typography_not_applicable() {
    let theme =
        flowchart_node_label_font_stack_and_size_theme_with_base(DiagramFamilyId::FLOWCHART, None);
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: start }
"#,
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict no-label NodeLabel session"),
    )
    .expect("prepare no-label Flowchart node")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("no-label node should not require NodeLabel typography");

    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[
            FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            },
            FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::NodeLabel,
            },
        ]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn flowchart_escaped_font_size_is_not_a_node_label_override() {
    let theme =
        flowchart_node_label_font_stack_and_size_theme_with_base(DiagramFamilyId::FLOWCHART, None);
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
classDef local f\6f nt-size:22px
A[Alpha]:::local
"#;
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort escaped font-size session"),
    )
    .expect("prepare escaped font-size Flowchart")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve escaped source CSS");

    let label_style = flowchart_node_label_style(rendered.svg(), "A");
    assert!(
        label_style.contains("font-family:\"Excalifont\" !important"),
        "{label_style}"
    );
    assert!(label_style.contains("font-size:26px !important"));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "A"
            && residual.property() == Some("font-size")
            && residual.channel() == FamilyStyleChannel::Shape
            && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
    }));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[
            FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            },
            FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::NodeLabel,
            },
        ],
        "svg={} report={:?}",
        rendered.svg(),
        rendered.style_report(),
    );

    let strict_result = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict escaped font-size session"),
    )
    .expect("escaped source evidence is completed during SVG emission")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
    let strict_error = match strict_result {
        Ok(_) => panic!("escaped shape CSS must remain fail-closed"),
        Err(error) => error,
    };
    assert_eq!(
        strict_error.unverified_family_style(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().residuals().len(),
        ))
    );
}

#[test]
fn flowchart_uppercase_font_size_is_not_a_node_label_override() {
    let theme =
        flowchart_node_label_font_stack_and_size_theme_with_base(DiagramFamilyId::FLOWCHART, None);
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
classDef local FONT-SIZE:22px
A[Alpha]:::local
"#,
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort uppercase font-size session"),
    )
    .expect("prepare uppercase font-size Flowchart")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve uppercase source CSS");

    let label_style = flowchart_node_label_style(rendered.svg(), "A");
    assert!(label_style.contains("font-size:26px !important"));
    assert!(!label_style.contains("font-size:22px"), "{label_style}");
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "A"
            && residual.property() == Some("font-size")
            && residual.channel() == FamilyStyleChannel::Shape
            && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
    }));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[
            FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            },
            FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::NodeLabel,
            },
        ]
    );
}

#[test]
fn flowchart_entity_authored_unicode_space_consumes_typed_node_label_typography() {
    let theme =
        flowchart_node_label_font_stack_and_size_theme_with_base(DiagramFamilyId::FLOWCHART, None);
    let source = "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA[\"&nbsp;\"]\n";
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict visible Unicode-space session"),
    )
    .expect("prepare visible Unicode-space Flowchart")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("prepared writer should prove visible Unicode-space typography");

    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[
            FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            },
            FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::NodeLabel,
            },
        ],
        "svg={} report={:?}",
        rendered.svg(),
        rendered.style_report(),
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert!(flowchart_node_label_style(rendered.svg(), "A").contains("font-size:26px !important"));
}

#[test]
fn flowchart_explicit_primary_color_owns_derived_main_background() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
        MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": {"primaryColor": "#123456"}
        })),
    ));
    let parsed = engine
        .parse_diagram_for_render_model_sync("flowchart LR\nA[Alpha]\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict render session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare derived Mermaid config")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("derived Mermaid main background should supersede typed Node paint");

    assert!(rendered.svg().contains("#123456"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("fill:#ef4444 !important"));
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn flowchart_theme_recipe_mermaid_config_owns_typed_node_paint_precedence() {
    let compatibility = MermaidThemeCompatibility::default()
        .with_variable("mainBkg", "#22c55e")
        .expect("valid Mermaid compatibility color");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                )
                .with_mermaid_compatibility(compatibility),
        )
        .expect("compile Flowchart theme recipe");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA[Alpha]\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin render session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare recipe compatibility precedence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort compatibility output");

    assert!(rendered.svg().contains("#22c55e"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("fill:#ef4444 !important"));
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
}

#[test]
fn flowchart_source_paint_residual_is_independent_from_theme_channel() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
    );
    let source = "flowchart LR\nstyle A --paint:#22c55e,fill:var(--paint)\nA[Alpha]\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort session"),
    )
    .expect("prepare dynamic source paint")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output must preserve dynamic source paint");

    assert!(rendered.svg().contains("fill:var(--paint) !important"));
    assert!(rendered.svg().contains("stroke:#2563eb !important"));
    assert!(rendered.style_report().theme_applied_mechanisms().contains(
        &FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }
    ));
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert_eq!(rendered.style_report().residuals().len(), 1);
    assert_eq!(rendered.style_report().residuals()[0].owner_id(), "A");
    assert_eq!(
        rendered.style_report().residuals()[0].property(),
        Some("fill")
    );

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict session"),
    )
    .expect("source residual is discovered during SVG emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("independent source residual must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().residuals().len(),
        ))
    );
}

#[test]
fn flowchart_unverified_node_surfaces_remain_fail_closed_after_svg_emission() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("rgb(239 68 68)").unwrap()),
    );
    let cases = [
        r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
A[Alpha]
"#,
        "flowchart LR\nA@{ shape: choice }\n",
    ];

    for source in cases {
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare unverified node surface")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort node surface should retain evidence");
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                },
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            }]
        );

        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");
        let artifact = prepare(parse(), &LayoutOptions::default(), session)
            .expect("concrete paint support must be decided during SVG emission");

        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unverified node surface must remain fail-closed"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((
                DiagramFamilyId::FLOWCHART,
                rendered.style_report().theme_residuals().len(),
            ))
        );
    }
}

#[test]
fn flowchart_start_node_proves_direct_typed_paint_emission() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "flowchart LR\nA@{ shape: start }\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart start source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("prepare Flowchart start")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("direct start paint should satisfy strict portability");

    assert!(rendered.svg().contains(
        "class=\"state-start\" r=\"7\" width=\"14\" height=\"14\" style=\"fill:#ef4444 !important\""
    ));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
}

#[test]
fn class_relation_stroke_width_is_emitted_and_verified_after_terminal_svg() {
    const SOURCE: &str = r#"classDiagram
class A
class B
class C
class D
A <|-- B
C <|.. D
note for A "note"
"#;

    for variant in [None, Some(crate::diagram_theme::ThemeVariant::Default)] {
        let theme = class_relation_theme(
            ThemeStylePatch::default()
                .with_stroke_width(6.0)
                .expect("valid Class relation width"),
            variant,
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict Class render session"),
        )
        .expect("Class direct relation width should wait for SVG evidence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("terminal Class relation width should satisfy strict portability");

        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Class SVG");
        let paths = document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 3, "expected two relations and one note edge");
        let relation_paths = paths
            .iter()
            .filter(|path| {
                !path
                    .attribute("data-id")
                    .is_some_and(|id| id.starts_with("edgeNote"))
            })
            .collect::<Vec<_>>();
        assert_eq!(relation_paths.len(), 2);
        assert!(relation_paths.iter().all(|path| {
            path.attribute("style")
                .is_some_and(|style| style.contains("stroke-width:6px !important"))
        }));
        assert!(relation_paths.iter().any(|path| {
            path.attribute("class")
                .is_some_and(|classes| classes.contains("edge-pattern-solid"))
        }));
        assert!(relation_paths.iter().any(|path| {
            path.attribute("class")
                .is_some_and(|classes| classes.contains("edge-pattern-dashed"))
        }));
        let note_path = paths
            .iter()
            .find(|path| {
                path.attribute("data-id")
                    .is_some_and(|id| id.starts_with("edgeNote"))
            })
            .expect("Class note edge path");
        assert!(
            !note_path
                .attribute("style")
                .is_some_and(|style| style.contains("stroke-width:6px"))
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
    }
}

#[test]
fn class_source_and_site_config_own_relation_stroke_width_precedence() {
    const DIAGRAM: &str = r#"classDiagram
direction LR
A o-- B
"#;
    const SOURCE_CONFIG: &str = r#"%%{init: {"themeVariables": {"strokeWidth": 160}}}%%
classDiagram
direction LR
A o-- B
"#;

    let theme = class_relation_theme(
        ThemeStylePatch::default()
            .with_stroke_width(6.0)
            .expect("valid Class relation width"),
        None,
    );
    let render = |engine: Engine, source: &str| {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class precedence source should produce a render model");
        prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict Class precedence session"),
        )
        .expect("prepare Class precedence fixture")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("source-owned Class relation width should supersede the typed default")
    };
    let view_box = |svg: &str| {
        roxmltree::Document::parse(svg)
            .expect("valid Class SVG")
            .root_element()
            .attribute("viewBox")
            .expect("Class root viewBox")
            .split_ascii_whitespace()
            .map(|part| part.parse::<f64>().expect("numeric viewBox component"))
            .collect::<Vec<_>>()
    };

    let typed = render(Engine::new(), DIAGRAM);
    let typed_view_box = view_box(typed.svg());
    assert!(typed.svg().contains("stroke-width:6px !important"));

    let configured = [
        render(
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "secure": [
                    "secure",
                    "securityLevel",
                    "startOnLoad",
                    "maxTextSize",
                    "suppressErrorRendering",
                    "maxEdges"
                ]
            }))),
            SOURCE_CONFIG,
        ),
        render(
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {"strokeWidth": 160}
            }))),
            DIAGRAM,
        ),
    ];
    for rendered in &configured {
        assert!(rendered.svg().contains("stroke-width:160;fill:none;}"));
        assert!(!rendered.svg().contains("stroke-width:6px !important"));
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert!(rendered.style_report().theme_residuals().is_empty());

        let configured_view_box = view_box(rendered.svg());
        assert!(
            configured_view_box[2] > typed_view_box[2] + 1_000.0,
            "typed={typed_view_box:?} configured={configured_view_box:?}"
        );
        assert!(
            configured_view_box[3] > typed_view_box[3] + 1_000.0,
            "typed={typed_view_box:?} configured={configured_view_box:?}"
        );
    }
}

#[test]
fn class_relation_stroke_width_clear_and_mixed_facets_fail_closed() {
    const SOURCE: &str = "classDiagram\nclass A\nclass B\nA --> B\n";

    let mut clear = ThemeStylePatch::default();
    clear.stroke.width = Specified::Clear;
    let mixed = ThemeStylePatch {
        geometry: ThemeGeometryPatch {
            radius: Specified::Value(8.0),
        },
        ..ThemeStylePatch::default()
            .with_stroke_width(6.0)
            .expect("valid Class relation width")
    };

    for (style, expects_width) in [(clear, false), (mixed, true)] {
        let theme = class_relation_theme(style, None);
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
                .unwrap()
                .expect("Class source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort Class render session"),
        )
        .expect("prepare best-effort Class relation theme")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort Class output should retain explicit residual evidence");

        assert_eq!(
            rendered.svg().contains("stroke-width:6px !important"),
            expects_width
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(rendered.style_report().theme_residuals().len(), 1);
        assert_eq!(
            rendered.style_report().theme_residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict Class render session"),
        )
        .expect("Class strict verification must wait for terminal SVG evidence");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("Clear or mixed unsupported Class facets must fail closed"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::CLASS, 1))
        );
    }
}

#[test]
fn class_relation_theme_is_byte_stable_without_a_selected_width() {
    const SOURCE: &str = r#"classDiagram
class A
class B
class C
class D
A <|-- B
C <|.. D
note for A "note"
"#;
    let empty_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty Class theme");

    let render = |engine: Engine, session: RenderSession| {
        let parsed = engine
            .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare Class byte-stability fixture")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render Class byte-stability fixture")
    };
    let unthemed = render(
        Engine::new(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin unthemed Class session"),
    );
    let themed = render(
        empty_theme.install_parse_compatibility(Engine::new()),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&empty_theme)
            .expect("begin empty-theme Class session"),
    );

    assert_eq!(themed.svg().as_bytes(), unthemed.svg().as_bytes());
    assert_eq!(
        themed.style_report().verification(),
        FamilyStyleVerification::NotApplicable
    );
}

#[test]
fn class_relation_stroke_width_includes_start_and_end_marker_paint_bounds() {
    const SOURCE: &str = r#"classDiagram
direction LR
A o-- B
C --* D
E <|.. F
G ..> H
"#;
    let theme = class_relation_theme(
        ThemeStylePatch::default()
            .with_stroke_width(160.0)
            .expect("valid wide Class relation stroke"),
        None,
    );
    let render = |engine: Engine, session: RenderSession| {
        let parsed = engine
            .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare Class paint-bounds fixture")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render Class paint-bounds fixture")
    };
    let baseline = render(
        Engine::new(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin unthemed Class session"),
    );
    let themed = render(
        theme.install_parse_compatibility(Engine::new()),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Class paint-bounds session"),
    );
    let view_box = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid Class SVG");
        document
            .root_element()
            .attribute("viewBox")
            .expect("Class root viewBox")
            .split_ascii_whitespace()
            .map(|part| part.parse::<f64>().expect("numeric viewBox component"))
            .collect::<Vec<_>>()
    };
    let baseline = view_box(baseline.svg());
    let themed = view_box(themed.svg());

    assert_eq!(baseline.len(), 4);
    assert_eq!(themed.len(), 4);
    assert!(
        themed[2] > baseline[2] + 1_000.0,
        "baseline={baseline:?} themed={themed:?}"
    );
    assert!(
        themed[3] > baseline[3] + 1_000.0,
        "baseline={baseline:?} themed={themed:?}"
    );
}

#[test]
fn class_relation_theme_is_not_applicable_without_relations() {
    let theme = class_relation_theme(
        ThemeStylePatch::default()
            .with_stroke_width(6.0)
            .expect("valid Class relation width"),
        None,
    );
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("classDiagram\nclass A\n", ParseOptions::strict())
        .unwrap()
        .expect("Class source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Class no-relation session"),
    )
    .expect("prepare Class no-relation fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("a Class Edge rule is not applicable without relations");

    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
}

#[test]
fn class_ordinal_relation_rules_use_concrete_one_based_applicability() {
    const ONE_RELATION: &str = "classDiagram\nclass A\nclass B\nA --> B\n";
    const TWO_RELATIONS: &str = "classDiagram\nclass A\nclass B\nclass C\nA --> B\nB --> C\n";
    let theme_for = |selector| {
        class_rule_theme(
            ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default()
                    .with_stroke_width(6.0)
                    .expect("valid ordinal Class relation width"),
            )
            .with_ordinal(selector)
            .for_family(DiagramFamilyId::CLASS),
        )
    };

    for (selector, source, applies) in [
        (OrdinalSelector::exact(1).unwrap(), ONE_RELATION, true),
        (OrdinalSelector::exact(2).unwrap(), ONE_RELATION, false),
        (OrdinalSelector::cycle(2, 1).unwrap(), TWO_RELATIONS, true),
    ] {
        let theme = theme_for(selector);
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class ordinal source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort Class ordinal session"),
        )
        .expect("prepare Class ordinal fixture")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render best-effort Class ordinal fixture");

        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        if applies {
            assert_eq!(rendered.style_report().theme_residuals().len(), 1);
            assert_eq!(
                rendered.style_report().theme_residuals()[0].reason(),
                FamilyThemeResidualReason::UnsupportedGeometry
            );
        } else {
            assert!(rendered.style_report().theme_residuals().is_empty());
            assert_eq!(
                rendered.style_report().theme_not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Edge,
                }]
            );
            assert_eq!(
                rendered.style_report().verification(),
                FamilyStyleVerification::Verified
            );
        }
    }

    let theme = theme_for(OrdinalSelector::exact(1).unwrap());
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(ONE_RELATION, ParseOptions::strict())
        .unwrap()
        .expect("Class exact ordinal source should produce a render model");
    let artifact = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Class exact ordinal session"),
    )
    .expect("strict ordinal verification must wait for Class SVG evidence");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("an applicable unsupported Class ordinal must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::CLASS, 1))
    );
}

#[test]
fn class_node_unsupported_rules_use_node_not_relation_applicability() {
    let theme = class_rule_theme(
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch {
                geometry: ThemeGeometryPatch {
                    radius: Specified::Value(8.0),
                },
                ..ThemeStylePatch::default()
            },
        )
        .for_family(DiagramFamilyId::CLASS),
    );
    let parse = |source| {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class Node source should produce a render model")
    };

    let rendered = prepare(
        parse("classDiagram\nclass A\n"),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort Class Node session"),
    )
    .expect("prepare Class Node fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render best-effort Class Node fixture");
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(rendered.style_report().theme_residuals().len(), 1);
    assert_eq!(
        rendered.style_report().theme_residuals()[0].reason(),
        FamilyThemeResidualReason::UnsupportedGeometry
    );

    let artifact = prepare(
        parse("classDiagram\nclass A\n"),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Class Node session"),
    )
    .expect("strict Node verification must wait for Class SVG evidence");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("an applicable unsupported Class Node rule must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::CLASS, 1))
    );

    let empty = prepare(
        parse("classDiagram\n"),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict empty Class Node session"),
    )
    .expect("prepare empty Class Node fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("a Class Node rule must be NotApplicable without nodes");
    assert_eq!(
        empty.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
    assert_eq!(
        empty.style_report().verification(),
        FamilyStyleVerification::Verified
    );

    let radius_rule = |radius| {
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch {
                geometry: ThemeGeometryPatch {
                    radius: Specified::Value(radius),
                },
                ..ThemeStylePatch::default()
            },
        )
        .for_family(DiagramFamilyId::CLASS)
    };
    let superseded_theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(radius_rule(4.0))
                    .with_rule(radius_rule(8.0)),
            ),
        )
        .expect("compile superseded Class Node rules");
    let parsed = superseded_theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("classDiagram\nclass A\n", ParseOptions::strict())
        .unwrap()
        .expect("superseded Class Node source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&superseded_theme)
            .expect("begin superseded Class Node session"),
    )
    .expect("prepare superseded Class Node fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render superseded Class Node fixture");
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
    assert_eq!(rendered.style_report().theme_residuals().len(), 1);
    assert_eq!(
        rendered.style_report().theme_residuals()[0].key(),
        &FamilyThemeMechanismKey::Rule {
            index: 1,
            target: ThemeTarget::Node,
        }
    );
}

#[test]
fn class_mixed_direct_width_and_legacy_paint_is_not_reported_as_applied() {
    let theme = class_rule_theme(
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#2563eb").unwrap())
                .with_stroke_width(6.0)
                .expect("valid mixed Class relation style"),
        )
        .for_family(DiagramFamilyId::CLASS),
    );
    const SOURCE: &str = "classDiagram\nclass A\nclass B\nA --> B\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
            .unwrap()
            .expect("mixed Class relation source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort mixed Class session"),
    )
    .expect("prepare mixed Class relation fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render best-effort mixed Class relation fixture");

    assert!(rendered.svg().contains("stroke-width:6px !important"));
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert!(rendered.style_report().compatibility_residual_count() > 0);

    let error = match prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict mixed Class session"),
    ) {
        Ok(_) => panic!("legacy Class paint must reject before false Applied evidence"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::LegacyFamilyThemeCompatibility {
            family_id: DiagramFamilyId::CLASS,
            ..
        }
    ));
}

#[test]
fn flowchart_common_style_node_preserves_best_effort_paint_and_source_precedence() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let render = |source: &str, portability| {
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart choice source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(portability)
            .begin_session_with_theme(&theme)
            .expect("begin Flowchart choice render session");
        prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare Flowchart choice")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    };

    let themed = render(
        "flowchart LR\nA@{ shape: choice }\n",
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort choice output should preserve typed paint");
    assert!(themed.svg().contains("fill:#ef4444 !important"));
    assert!(themed.style_report().residuals().is_empty());
    assert_eq!(themed.style_report().theme_residuals().len(), 1);
    assert_eq!(
        themed.style_report().theme_residuals()[0].reason(),
        FamilyThemeResidualReason::UnsupportedPaint
    );

    let sourced = render(
        "flowchart LR\nstyle A fill:#22c55e\nA@{ shape: choice }\n",
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("verified static source paint should supersede typed choice paint");
    assert!(sourced.svg().contains("fill:#22c55e !important"));
    assert!(!sourced.svg().contains("fill:#ef4444 !important"));
    assert!(sourced.style_report().residuals().is_empty());
    assert!(sourced.style_report().theme_residuals().is_empty());
    assert_eq!(
        sourced.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
}

#[test]
fn flowchart_unemitted_source_paint_cannot_hide_theme_residual() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    for source in [
        "flowchart LR\nstyle A fill:#22c55e\nA@{ shape: start }\n",
        "flowchart LR\nstyle A fill:#22c55e\nA@{ shape: icon, label: \"Plain\" }\n",
    ] {
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare Flowchart source")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve Mermaid emission semantics");

        assert!(!rendered.svg().contains("fill:#22c55e !important"));
        assert!(!rendered.svg().contains("fill:#ef4444 !important"));
        assert_eq!(rendered.style_report().residuals().len(), 1);
        assert_eq!(
            rendered.style_report().residuals()[0].reason(),
            FamilyStyleResidualReason::UnsupportedSurface
        );
        assert_eq!(rendered.style_report().theme_residuals().len(), 1);
        assert_eq!(
            rendered.style_report().theme_residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("strict verification must wait for SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unemitted paint must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((
                DiagramFamilyId::FLOWCHART,
                rendered.style_report().theme_residuals().len(),
            ))
        );
    }
}

#[test]
fn flowchart_generated_default_class_css_is_a_real_start_paint_surface() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "flowchart LR\nclassDef default fill:#22c55e\nA@{ shape: start }\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict render session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("generated default class paint is decided during SVG emission")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("static generated class paint should remain portable");

    assert!(rendered.svg().contains("fill:rgb(34, 197, 94)!important"));
    assert!(!rendered.svg().contains("fill:#ef4444 !important"));
    assert!(rendered.style_report().residuals().is_empty());
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
}

#[test]
fn flowchart_start_uses_the_generated_css_winner_that_reaches_its_wrapper() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nclassDef default fill:#22c55e\nclassDef explicit fill:#f97316\nA@{ shape: start }\nclass A explicit\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict render session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("generated CSS winner is decided during SVG emission")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("the reachable static default class remains portable");

    assert!(rendered.svg().contains("fill:rgb(34, 197, 94)!important"));
    assert!(rendered.svg().contains("fill:rgb(249, 115, 22)!important"));
    assert!(!rendered.svg().contains("fill:#ef4444 !important"));
    assert!(rendered.style_report().residuals().is_empty());
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
}

#[test]
fn flowchart_anchor_dynamic_generated_css_is_fail_closed() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty typed theme");
    let source = "flowchart LR\nclassDef default opacity:var(--alpha)\nA@{ shape: anchor }\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort session"),
    )
    .expect("prepare generated CSS fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve generated CSS");

    assert!(
        rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "default"
                && residual.property() == Some("opacity")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }),
        "residuals={:?}",
        rendered.style_report().residuals()
    );

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict session"),
    )
    .expect("generated CSS verification waits for emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic generated CSS must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().residuals().len(),
        ))
    );
}

#[test]
fn flowchart_generated_css_emits_important_once() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            "flowchart LR\nclassDef default fill:#22c55e !important\nA@{ shape: start }\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let rendered = prepare(parsed, &LayoutOptions::default(), session())
        .expect("prepare important fixture")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render important fixture");

    assert!(rendered.svg().contains("fill:rgb(34, 197, 94)!important"));
    assert!(!rendered.svg().contains("important!important"));
}

#[test]
fn flowchart_direct_class_style_outranks_generated_class_css() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source = "flowchart LR\nclassDef z fill:var(--paint)\nclassDef default fill:#22c55e\nA[Alpha]:::z\nstyle A --paint:#f97316\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare class precedence fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should retain the direct class winner");

    assert!(rendered.svg().contains("fill:var(--paint) !important"));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "A"
            && residual.property() == Some("fill")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("source precedence is decided during SVG emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic direct class paint must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().residuals().len(),
        ))
    );
}

#[test]
fn flowchart_hand_drawn_keeps_dynamic_source_paint_in_the_render_transport() {
    let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"look\": \"handDrawn\", \"handDrawnSeed\": 7}}%%\nflowchart LR\nstyle A --paint:#22c55e,fill:var(--paint)\nA[Alpha]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare hand-drawn Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should retain the dynamic paint transport");

    assert!(rendered.svg().contains("stroke=\"var(--paint)\""));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "A"
            && residual.property() == Some("fill")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
}

#[test]
fn flowchart_raw_theme_css_invalidates_family_theme_evidence() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source =
        "%%{init: {\"themeCSS\": \".node rect { fill: #22c55e; }\"}}%%\nflowchart LR\nA[Alpha]\n";
    let environment = || {
        crate::environment::RenderEnvironment::deterministic().with_theme_admission_policy(
            crate::diagram_theme::ThemeAdmissionPolicy::permissive().with_trusted_lanes(
                crate::diagram_theme::TrustedThemeLanes::from_allowed([
                    crate::diagram_theme::TrustedThemeLane::RawThemeCss,
                ]),
            ),
        )
    };
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new().with_site_config(MermaidConfig::from_value(
                json!({
                    "secure": [
                        "secure",
                        "securityLevel",
                        "startOnLoad",
                        "maxTextSize",
                        "suppressErrorRendering",
                        "maxEdges"
                    ]
                }),
            )))
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        environment()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare raw theme CSS")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output may retain raw theme CSS");
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(rendered.style_report().theme_residuals().len(), 1);
    assert_eq!(
        rendered.style_report().theme_residuals()[0].reason(),
        FamilyThemeResidualReason::OutputMutation
    );
    assert!(rendered.style_report().output_mutated());

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        environment()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("raw theme CSS is evaluated after SVG emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("raw theme CSS must invalidate strict family evidence"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::UnverifiedFamilyOutputMutation {
            family_id: DiagramFamilyId::FLOWCHART
        }
    ));
}

#[test]
fn raw_theme_css_rejects_strict_output_when_typed_rules_are_not_applicable() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source =
        "%%{init: {\"themeCSS\": \".node rect { fill: #f97316; }\"}}%%\nflowchart LR\nA[Alpha]\n";
    let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
        MermaidConfig::from_value(json!({
            "secure": [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges"
            ],
            "themeVariables": {"mainBkg": "#22c55e"}
        })),
    ));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_admission_policy(
            crate::diagram_theme::ThemeAdmissionPolicy::permissive().with_trusted_lanes(
                crate::diagram_theme::TrustedThemeLanes::from_allowed([
                    crate::diagram_theme::TrustedThemeLane::RawThemeCss,
                ]),
            ),
        )
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict render session");
    let artifact = prepare(parsed, &LayoutOptions::default(), session)
        .expect("raw theme CSS mutation is observed after SVG emission");

    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("raw theme CSS must reject strict output independently of rule state"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::UnverifiedFamilyOutputMutation {
            family_id: DiagramFamilyId::FLOWCHART
        }
    ));
}

#[test]
fn flowchart_emitted_dynamic_shape_property_is_a_source_residual() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source = "flowchart LR\nstyle A stroke-width:var(--width)\nA[Alpha]\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare dynamic source property")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve dynamic source property");

    assert!(
        rendered
            .svg()
            .contains("stroke-width:var(--width) !important")
    );
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "A"
            && residual.property() == Some("stroke-width")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("source residual is discovered during SVG emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic source property must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().residuals().len(),
        ))
    );
}

#[test]
fn flowchart_edge_dynamic_shape_style_is_a_source_residual() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source = "flowchart LR\nA --> B\nlinkStyle 0 stroke:var(--marker),stroke:#111827,stroke-width:var(--width),opacity:var(--alpha),filter:url(#alpha)\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare dynamic edge style")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve dynamic edge style");

    assert!(rendered.svg().contains("stroke-width:var(--width)"));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "L_A_B_0"
            && residual.property() == Some("stroke-width")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "L_A_B_0"
            && residual.property() == Some("opacity")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "L_A_B_0" && residual.raw().contains("filter:url(#alpha)")
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "L_A_B_0" && residual.raw().contains("stroke:var(--marker)")
    }));

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("strict verification must wait for edge emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic edge style must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style().map(|(family, _)| family),
        Some(DiagramFamilyId::FLOWCHART)
    );
}

#[test]
fn flowchart_assigned_class_marker_style_is_a_source_residual() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source =
        "flowchart LR\nA edge@--> B\nclassDef dynamic stroke:var(--edge)\nclass edge dynamic\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare assigned-class marker style")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve assigned-class marker style");

    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "edge"
            && residual.class_id() == Some("dynamic")
            && residual.origin() == FamilyStyleOrigin::AssignedClass
            && residual.property() == Some("stroke")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("strict verification must wait for marker emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic assigned-class marker must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style().map(|(family, _)| family),
        Some(DiagramFamilyId::FLOWCHART)
    );
}

#[test]
fn flowchart_invalid_assigned_edge_class_declaration_is_a_source_residual() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source =
        "flowchart LR\nA edge@--> B\nclassDef unsafe filter:url(#alpha)\nclass edge unsafe\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare invalid assigned edge class declaration")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should report the dropped declaration");

    assert!(!rendered.svg().contains("filter:url(#alpha)"));
    let residual = rendered
        .style_report()
        .residuals()
        .iter()
        .find(|residual| {
            residual.owner_id() == "edge"
                && residual.class_id() == Some("unsafe")
                && residual.raw() == "filter:url(#alpha)"
        })
        .expect("invalid assigned declaration residual");
    assert_eq!(residual.property(), None);
    assert_eq!(residual.origin(), FamilyStyleOrigin::AssignedClass);
    assert_eq!(residual.channel(), FamilyStyleChannel::Shape);
    assert_eq!(residual.assignment_ordinal(), Some(0));
    assert_eq!(residual.declaration_ordinal(), 0);
    assert_eq!(
        residual.reason(),
        FamilyStyleResidualReason::InvalidDeclaration
    );

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("strict verification must wait for edge emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("invalid assigned edge declaration must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style().map(|(family, _)| family),
        Some(DiagramFamilyId::FLOWCHART)
    );
}

#[test]
fn flowchart_edge_label_invalid_typography_is_a_source_residual() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source = "flowchart LR\nA -->|label| B\nlinkStyle 0 font-weight:banana\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare dynamic edge label style")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve dynamic edge label style");

    assert!(rendered.svg().contains("font-weight:banana"));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "L_A_B_0"
            && residual.property() == Some("font-weight")
            && residual.channel() == FamilyStyleChannel::Label
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("strict verification must wait for edge label emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic edge label style must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style().map(|(family, _)| family),
        Some(DiagramFamilyId::FLOWCHART)
    );
}

#[test]
fn flowchart_edge_sanitized_xhtml_and_empty_label_styles_are_source_residuals() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source = r#"---
config:
  securityLevel: loose
  htmlLabels: true
  flowchart:
    htmlLabels: true
---
flowchart LR
A -->|"<span class='host-label' style='color:var(--accent)'>Label</span>"| B
B --> C
linkStyle 1 font-weight:banana
"#;
    let rendered = prepare(
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model"),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare HTML edge labels")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve sanitized XHTML styles");

    assert!(rendered.svg().contains("class='host-label'"));
    assert!(rendered.svg().contains("style='color:var(--accent)'"));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "L_A_B_0"
            && residual.property() == Some("color")
            && residual.origin() == FamilyStyleOrigin::LabelStyle
            && residual.channel() == FamilyStyleChannel::Label
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "L_A_B_0"
            && residual.property() == Some("class")
            && residual.origin() == FamilyStyleOrigin::LabelStyle
            && residual.channel() == FamilyStyleChannel::Label
            && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "L_B_C_0"
            && residual.property() == Some("font-weight")
            && residual.channel() == FamilyStyleChannel::Label
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
}

#[test]
fn flowchart_cluster_dynamic_shape_style_is_a_source_residual() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source = "flowchart TD\nsubgraph S[Service]\nA\nend\nstyle S stroke-width:var(--width),transform:translateX(1px)\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare dynamic cluster style")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve dynamic cluster style");

    assert!(rendered.svg().contains("stroke-width:var(--width)"));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "S"
            && residual.property() == Some("stroke-width")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "S"
            && residual.property() == Some("transform")
            && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
    }));

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("strict verification must wait for cluster emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic cluster style must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style().map(|(family, _)| family),
        Some(DiagramFamilyId::FLOWCHART)
    );
}

#[test]
fn require_portable_accepts_hand_drawn_cluster_emitted_shape_facets() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty strict theme");

    for (family, source) in [
        (
            DiagramFamilyId::FLOWCHART,
            r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart TD
subgraph Group[Group]
A
end
style Group fill:#f8fafc,stroke:#ef4444,stroke-width:2px,stroke-dasharray:4 2
"#,
        ),
        (
            DiagramFamilyId::SWIMLANE,
            r#"---
config:
  layout: swimlane
  look: handDrawn
  handDrawnSeed: 7
---
flowchart TD
subgraph Lane[Lane]
A
end
style Lane fill:#f8fafc,stroke:#ef4444,stroke-width:2px,stroke-dasharray:4 2
"#,
        ),
    ] {
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("hand-drawn cluster source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict hand-drawn cluster session"),
        )
        .expect("prepare hand-drawn cluster")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("emitted hand-drawn cluster facets should satisfy strict portability");

        assert_eq!(rendered.family_id(), family);
        assert!(rendered.svg().contains("<path "), "{}", rendered.svg());
        for emitted_declaration in [
            "stroke:#f8fafc !important",
            "stroke:#ef4444 !important",
            "stroke-width:2px !important",
            "stroke-dasharray:4 2 !important",
        ] {
            assert!(
                rendered.svg().contains(emitted_declaration),
                "missing emitted {emitted_declaration} for {family}: {}",
                rendered.svg(),
            );
        }
        assert!(
            rendered.style_report().residuals().is_empty(),
            "unexpected {family} residuals: {:#?}",
            rendered.style_report().residuals(),
        );
    }
}

#[test]
fn flowchart_cluster_sanitized_xhtml_and_empty_title_styles_are_source_residuals() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source = r#"---
config:
  securityLevel: loose
  htmlLabels: true
  flowchart:
    htmlLabels: true
---
flowchart TD
subgraph Rich["<span class='host-title'>Service</span>"]
A
end
subgraph Empty[" "]
B
end
style Empty font-weight:banana
"#;
    let rendered = prepare(
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model"),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare HTML cluster titles")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve sanitized cluster XHTML");

    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "Rich"
            && residual.property() == Some("class")
            && residual.origin() == FamilyStyleOrigin::LabelStyle
            && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "Empty"
            && residual.property() == Some("font-weight")
            && residual.channel() == FamilyStyleChannel::Label
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
}

#[test]
fn swimlane_cluster_dynamic_shape_style_is_a_source_residual() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SWIMLANE),
                ),
            ),
        )
        .expect("compile Swimlane Node theme");
    let source = "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nsubgraph Lane[Lane]\nA\nend\nstyle Lane opacity:var(--alpha),stroke-width:var(--width)\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Swimlane source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare dynamic Swimlane style")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve dynamic Swimlane style");

    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    assert!(rendered.svg().contains("opacity:var(--alpha)"));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "Lane"
            && residual.property() == Some("opacity")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "Lane"
            && residual.property() == Some("stroke-width")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session"),
    )
    .expect("strict verification must wait for Swimlane emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic Swimlane style must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style().map(|(family, _)| family),
        Some(DiagramFamilyId::SWIMLANE)
    );
}

#[test]
fn swimlane_sanitized_xhtml_and_empty_title_styles_are_source_residuals() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SWIMLANE),
                ),
            ),
        )
        .expect("compile Swimlane Node theme");
    let source = r#"---
config:
  layout: swimlane
  securityLevel: loose
  htmlLabels: true
  flowchart:
    htmlLabels: true
---
flowchart TD
subgraph Rich["<span style='color:var(--lane-title)'>Lane</span>"]
A
end
subgraph Empty[" "]
B
end
style Empty font-weight:banana
"#;
    let rendered = prepare(
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Swimlane source should produce a render model"),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare HTML Swimlane titles")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve sanitized Swimlane XHTML");

    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "Rich"
            && residual.property() == Some("color")
            && residual.origin() == FamilyStyleOrigin::LabelStyle
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "Empty"
            && residual.property() == Some("font-weight")
            && residual.channel() == FamilyStyleChannel::Label
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
}

#[test]
fn swimlane_empty_edge_label_still_records_emitted_shape_style() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SWIMLANE),
                ),
            ),
        )
        .expect("compile Swimlane Node theme");
    let source = "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA -->| | B\nlinkStyle 0 opacity:var(--alpha)\n";
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Swimlane source should produce a render model");

    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare empty Swimlane edge label")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve emitted edge label style");

    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.property() == Some("opacity")
            && residual.channel() == FamilyStyleChannel::Shape
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));
}

#[test]
fn flowchart_dynamic_label_typography_is_a_source_residual() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let source = "flowchart LR\nclassDef default --size:40px,font-size:var(--size)\nA[Alpha]\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort session"),
    )
    .expect("prepare dynamic label fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve label CSS");

    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "A"
            && residual.property() == Some("font-size")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict session"),
    )
    .expect("dynamic label verification waits for emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic label typography must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().residuals().len(),
        ))
    );
}

#[test]
fn flowchart_icon_label_typography_is_emitted_and_residualized() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty theme session");
    let source = "flowchart LR\nclassDef default --size:40px,font-size:var(--size)\nA@{ icon: \"missing:icon\", label: \"Alpha\" }\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort session"),
    )
    .expect("prepare icon label fixture")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort output should preserve icon label CSS");

    assert!(rendered.svg().contains("font-size:var(--size) !important"));
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.owner_id() == "A"
            && residual.property() == Some("font-size")
            && residual.reason() == FamilyStyleResidualReason::InvalidValue
    }));

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict session"),
    )
    .expect("dynamic icon label verification waits for emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("dynamic icon label typography must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().residuals().len(),
        ))
    );
}

#[test]
fn flowchart_empty_subgraph_consumes_typed_node_paint() {
    let theme = flowchart_node_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
    );
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "flowchart TD\nsubgraph Empty\nend\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("empty subgraph source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable render session");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("empty subgraph theme must reach SVG emission")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("emitted empty subgraph paint must satisfy strict portability");

    assert!(rendered.svg().contains("fill:#ef4444 !important"));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
}

#[test]
fn flowchart_icon_without_asset_retains_unemitted_fill_as_residual() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        )
        .expect("compile Flowchart icon theme");
    let source = "flowchart LR\nI@{ shape: icon, label: \"Plain\" }\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart icon source should produce a render model")
    };

    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare Flowchart icon")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render best-effort Flowchart icon");

    assert!(!rendered.svg().contains("#ef4444"));
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            },
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }]
    );

    let strict_session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict render session");
    let artifact = prepare(parse(), &LayoutOptions::default(), strict_session)
        .expect("strict verification must wait for icon emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("unemitted icon fill must fail strict portability"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().theme_residuals().len(),
        ))
    );
}

#[test]
fn require_portable_rejects_flowchart_unsupported_styles_after_svg_emission() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#0f172a").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#22d3ee").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let cases = [
        (
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::LinearGradient(gradient))
                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
            FamilyThemeResidualReason::UnsupportedPaint,
            "flowchart LR\nA --> B\n",
        ),
        (
            ThemeStylePatch {
                geometry: crate::diagram_theme::ThemeGeometryPatch {
                    radius: crate::diagram_theme::Specified::Value(8.0),
                },
                ..ThemeStylePatch::default()
            },
            FamilyThemeResidualReason::UnsupportedGeometry,
            "flowchart LR\nA@{ shape: choice }\n",
        ),
    ];

    for (style, expected_reason, source) in cases {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(ThemeTarget::Node, style).for_family(DiagramFamilyId::FLOWCHART),
                )),
            )
            .expect("compile unsupported Flowchart theme");
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare unsupported Flowchart theme")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort Flowchart output should retain residual evidence");
        assert_eq!(rendered.style_report().theme_residuals().len(), 1);
        assert_eq!(
            rendered.style_report().theme_residuals()[0].reason(),
            expected_reason
        );

        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");
        let artifact = prepare(parse(), &LayoutOptions::default(), session)
            .expect("Flowchart theme verification must wait for SVG emission");

        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unsupported Flowchart style must remain fail-closed"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((
                DiagramFamilyId::FLOWCHART,
                rendered.style_report().theme_residuals().len(),
            ))
        );
    }
}

#[test]
fn require_portable_accepts_flowchart_node_ordinal_palette_after_svg_emission() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    OrdinalPalette::new([
                        ThemeColorValue::parse("#ef4444").unwrap(),
                        ThemeColorValue::parse("#2563eb").unwrap(),
                    ])
                    .unwrap(),
                ),
            ),
        )
        .expect("compile Flowchart ordinal palette theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "flowchart LR\nA[Alpha] --> B[Beta]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session"),
    )
    .expect("prepare Flowchart ordinal palette")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("emitted ordinal palette must satisfy strict portability");

    assert!(flowchart_node_shape_style(rendered.svg(), "A").contains("fill:#ef4444 !important"));
    assert!(flowchart_node_shape_style(rendered.svg(), "B").contains("fill:#2563eb !important"));
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn require_portable_accepts_swimlane_node_ordinal_palette_after_svg_emission() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    OrdinalPalette::new([
                        ThemeColorValue::parse("#ef4444").unwrap(),
                        ThemeColorValue::parse("#2563eb").unwrap(),
                    ])
                    .unwrap(),
                ),
            ),
        )
        .expect("compile Swimlane ordinal palette theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha] --> B[Beta]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Swimlane source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session"),
    )
    .expect("prepare Swimlane ordinal palette")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("emitted Swimlane ordinal palette must satisfy strict portability");

    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    assert!(flowchart_node_shape_style(rendered.svg(), "A").contains("fill:#ef4444 !important"));
    assert!(flowchart_node_shape_style(rendered.svg(), "B").contains("fill:#2563eb !important"));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn require_portable_rejects_flowchart_ordinal_rule_when_a_node_matches() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .with_ordinal(OrdinalSelector::exact(2).unwrap())
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        )
        .expect("compile ordinal Flowchart rule");
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[Alpha] --> B[Beta]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort render session"),
    )
    .expect("prepare ordinal Flowchart rule")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort ordinal rule should retain residual evidence");
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            },
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }]
    );

    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable render session");
    let artifact = prepare(parse(), &LayoutOptions::default(), session)
        .expect("ordinal verification must wait for SVG emission");

    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("matching unsupported ordinal rule must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((
            DiagramFamilyId::FLOWCHART,
            rendered.style_report().theme_residuals().len(),
        ))
    );
}

#[test]
fn require_portable_accepts_flowchart_typed_edge_stroke_after_svg_emission() {
    let theme = flowchart_edge_stroke_theme(
        DiagramFamilyId::FLOWCHART,
        CanvasPaint::solid("#ef4444").unwrap(),
    );
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable render session");

    let artifact = prepare(parsed, &LayoutOptions::default(), session)
        .expect("typed Flowchart edge stroke must reach SVG emission");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("emitted typed Flowchart edge stroke must satisfy strict portability");

    assert!(flowchart_edge_path_style(rendered.svg()).contains("stroke:#ef4444 !important"));
    let marker_id = flowchart_edge_marker_id(rendered.svg());
    assert!(marker_id.ends_with("-pointEnd"));
    assert!(!flowchart_marker_contains(
        rendered.svg(),
        &marker_id,
        "#ef4444"
    ));
    assert!(
        rendered
            .svg()
            .contains(".marker{fill:#333333;stroke:#333333;}"),
        "typed Edge stroke must not leak into Marker CSS: {}",
        rendered.svg()
    );
    let report = rendered.style_report();
    assert_eq!(report.verification(), FamilyStyleVerification::Verified);
    assert_eq!(report.compatibility_residual_count(), 0);
    assert_eq!(
        report.theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        }]
    );
}

#[test]
fn require_portable_accepts_swimlane_transparent_edge_stroke_after_svg_emission() {
    let theme = flowchart_edge_stroke_theme(DiagramFamilyId::SWIMLANE, CanvasPaint::Transparent);
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: swimlane\n---\nflowchart LR\nA --> B\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Swimlane source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Swimlane session"),
    )
    .expect("prepare typed Swimlane edge stroke")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("emitted typed Swimlane edge stroke must satisfy strict portability");

    assert_eq!(rendered.family_id(), DiagramFamilyId::SWIMLANE);
    assert!(flowchart_edge_path_style(rendered.svg()).contains("stroke:none !important"));
    let marker_id = flowchart_edge_marker_id(rendered.svg());
    assert!(marker_id.ends_with("-pointEnd"));
    assert!(!flowchart_marker_contains(
        rendered.svg(),
        &marker_id,
        "none"
    ));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn require_portable_accepts_flowchart_and_swimlane_edge_dasharray_after_svg_emission() {
    for (family, source) in [
        (DiagramFamilyId::FLOWCHART, "flowchart LR\nA --> B\n"),
        (
            DiagramFamilyId::SWIMLANE,
            "---\nconfig:\n  layout: swimlane\n---\nflowchart LR\nA --> B\n",
        ),
    ] {
        let theme = flowchart_edge_stroke_dasharray_theme(family, [4.0, 2.0]);
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict edge dasharray session"),
        )
        .expect("prepare typed Edge stroke dasharray")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("terminal edge writer should prove typed stroke dasharray");

        assert_eq!(rendered.family_id(), family);
        assert!(
            flowchart_edge_path_style(rendered.svg()).contains("stroke-dasharray:4 2 !important")
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn flowchart_edge_source_dasharray_owns_typed_precedence() {
    let theme = flowchart_edge_stroke_dasharray_theme(DiagramFamilyId::FLOWCHART, [4.0, 2.0]);
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "flowchart LR\nA --> B\nlinkStyle 0 stroke-dasharray:8 3\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict edge dasharray precedence session"),
    )
    .expect("prepare source-owned Edge stroke dasharray")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("source-owned Edge stroke dasharray should satisfy strict portability");

    let style = flowchart_edge_path_style(rendered.svg());
    assert!(style.contains("stroke-dasharray:8 3"));
    assert!(!style.contains("stroke-dasharray:4 2 !important"));
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn flowchart_neo_edge_dasharray_is_the_terminal_style_winner() {
    let theme = flowchart_edge_stroke_dasharray_theme(DiagramFamilyId::FLOWCHART, [4.0, 2.0]);
    let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"look\": \"neo\"}}%%\nflowchart LR\nA --> B\nB animated@==> C\nanimated@{ animate: true }\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Neo Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Neo edge dasharray session"),
    )
    .expect("prepare Neo Edge stroke dasharray")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("Neo terminal writer should prove typed stroke dasharray");

    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Neo Flowchart SVG");
    let edge_style = |edge_id: &str| {
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-edge") == Some("true")
                    && node.attribute("data-id") == Some(edge_id)
            })
            .and_then(|node| node.attribute("style"))
            .unwrap_or_else(|| panic!("Neo edge style for {edge_id}"))
    };
    let static_style = edge_style("L_A_B_0");
    let mask = static_style
        .find("stroke-dasharray: 0 ")
        .expect("Neo marker-clearance mask");
    let typed = static_style
        .rfind("stroke-dasharray:4 2 !important")
        .expect("typed terminal dasharray");
    assert!(
        typed > mask,
        "typed dasharray must follow the Neo mask: {static_style}"
    );
    let animated_style = edge_style("animated");
    assert!(!animated_style.contains("stroke-dasharray: 0 "));
    assert!(animated_style.contains("stroke-dasharray:4 2 !important"));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn flowchart_hand_drawn_edge_dasharray_fails_closed() {
    let theme = flowchart_edge_stroke_dasharray_theme(DiagramFamilyId::FLOWCHART, [4.0, 2.0]);
    let source =
        "%%{init: {\"look\": \"handDrawn\", \"handDrawnSeed\": 7}}%%\nflowchart LR\nA --> B\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("hand-drawn Flowchart source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort hand-drawn dasharray session"),
    )
    .expect("prepare hand-drawn Edge stroke dasharray")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort hand-drawn output should retain residual evidence");

    assert!(!flowchart_edge_path_style(rendered.svg()).contains("stroke-dasharray:4 2 !important"));
    assert_eq!(rendered.style_report().theme_residuals().len(), 1);
    assert_eq!(
        rendered.style_report().theme_residuals()[0].reason(),
        FamilyThemeResidualReason::UnsupportedGeometry
    );

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict hand-drawn dasharray session"),
    )
    .expect("writer support is decided during SVG emission");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("hand-drawn Edge dasharray must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::FLOWCHART, 1))
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn elk_flowchart_reuses_the_typed_edge_dasharray_writer() {
    let theme = flowchart_edge_stroke_dasharray_theme(DiagramFamilyId::FLOWCHART, [4.0, 2.0]);
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: elk\n---\nflowchart LR\nA --> B\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("ELK Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict ELK dasharray session"),
    )
    .expect("prepare ELK Edge stroke dasharray")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("ELK must reuse the terminal Edge dasharray writer");

    assert!(flowchart_edge_path_style(rendered.svg()).contains("stroke-dasharray:4 2 !important"));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        }]
    );
}

#[test]
fn flowchart_edge_source_and_explicit_line_color_own_typed_precedence() {
    let theme = flowchart_edge_stroke_theme(
        DiagramFamilyId::FLOWCHART,
        CanvasPaint::solid("#ef4444").unwrap(),
    );
    let cases = [
        (
            theme.install_parse_compatibility(Engine::new()),
            "flowchart LR\nA edge@--> B\nlinkStyle 0 stroke:#22c55e\n",
            "stroke:#22c55e",
            Some("stroke:#22c55e"),
        ),
        (
            theme.install_parse_compatibility(Engine::new()),
            "flowchart LR\nA edge@--> B\nclassDef source stroke:#2563eb\nclass edge source\n",
            "stroke:#2563eb",
            Some("stroke:#2563eb"),
        ),
        (
            theme.install_parse_compatibility(Engine::new().with_site_config(
                MermaidConfig::from_value(json!({
                    "theme": "base",
                    "themeVariables": {"lineColor": "#16a34a"}
                })),
            )),
            "flowchart LR\nA --> B\n",
            "#16a34a",
            None,
        ),
    ];

    for (engine, source, expected_source, expected_path_style) in cases {
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict edge precedence session"),
        )
        .expect("prepare Flowchart edge precedence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("source/config edge stroke must satisfy strict portability");

        assert!(
            rendered.svg().contains(expected_source),
            "{}",
            rendered.svg()
        );
        if let Some(expected_path_style) = expected_path_style {
            assert!(
                flowchart_edge_path_style(rendered.svg()).contains(expected_path_style),
                "expected source-owned edge declaration on the terminal path: {}",
                flowchart_edge_path_style(rendered.svg())
            );
        } else {
            assert!(
                rendered
                    .svg()
                    .contains(".flowchart-link{stroke:#16a34a;fill:none;}"),
                "expected explicit lineColor on the edge's scoped CSS transport: {}",
                rendered.svg()
            );
        }
        assert!(!flowchart_edge_path_style(rendered.svg()).contains("stroke:#ef4444 !important"));
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn flowchart_hand_drawn_class_stroke_does_not_shadow_typed_edge_stroke() {
    let theme = flowchart_edge_stroke_theme(
        DiagramFamilyId::FLOWCHART,
        CanvasPaint::solid("#ef4444").unwrap(),
    );
    let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"look\": \"handDrawn\", \"handDrawnSeed\": 7}}%%\nflowchart LR\nA edge@--> B\nclassDef source stroke:#2563eb\nclass edge source\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("hand-drawn Flowchart source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort hand-drawn edge session"),
    )
    .expect("prepare hand-drawn Flowchart edge")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort hand-drawn edge output");

    let edge_style = flowchart_edge_path_style(rendered.svg());
    assert!(edge_style.contains("stroke:#ef4444 !important"));
    assert!(!edge_style.contains("#2563eb"));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        }]
    );
    assert!(rendered.style_report().residuals().iter().any(|residual| {
        residual.origin() == FamilyStyleOrigin::AssignedClass
            && residual.property() == Some("stroke")
            && residual.reason() == FamilyStyleResidualReason::UnsupportedSurface
    }));
}

#[test]
fn require_portable_accepts_sequence_actor_fill_after_svg_emission() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Sequence session"),
    )
    .expect("Sequence actor theme should prepare")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("Sequence actor fill should be proven by the SVG writer");

    assert!(
        rendered.svg().contains("fill:#ef4444"),
        "Sequence SVG should contain the typed actor fill"
    );
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn require_portable_accepts_sequence_actor_stroke_after_svg_emission() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor stroke theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Sequence session"),
    )
    .expect("Sequence actor stroke theme should prepare")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("Sequence actor stroke should be proven by the SVG writer");

    assert!(
        rendered.svg().contains("stroke:#2563eb"),
        "Sequence SVG should contain the typed actor stroke"
    );
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn require_portable_accepts_sequence_lifeline_scalar_paint_after_actor_line_emission() {
    for (facet, paint, expected_stroke) in [
        (
            "fill-solid",
            CanvasPaint::solid("#ef4444").expect("valid Lifeline fill"),
            "#ef4444",
        ),
        ("fill-transparent", CanvasPaint::Transparent, "transparent"),
        (
            "stroke-solid",
            CanvasPaint::solid("#2563eb").expect("valid Lifeline stroke"),
            "#2563eb",
        ),
        (
            "stroke-transparent",
            CanvasPaint::Transparent,
            "transparent",
        ),
    ] {
        let style = if facet.starts_with("fill") {
            ThemeStylePatch::default().with_fill(paint)
        } else {
            ThemeStylePatch::default().with_stroke(paint)
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(ThemeTarget::Lifeline, style)
                            .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Lifeline theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nactor Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence Lifeline session"),
        )
        .expect("Sequence Lifeline theme should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("every actor Lifeline should be proven by its terminal writer");

        assert!(
            rendered
                .svg()
                .contains(&format!("#merman .actor-line{{stroke:{expected_stroke};}}")),
            "{facet}: {}",
            rendered.svg()
        );
        assert_eq!(
            rendered.svg().matches(r#"data-et="life-line""#).count(),
            2,
            "{facet} must cover the actor-man and regular participant lifelines"
        );
        assert!(rendered.svg().contains(r#"data-id="Alice""#));
        assert!(rendered.svg().contains(r#"data-id="Bob""#));
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Lifeline,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn require_portable_accepts_sequence_lifeline_stroke_width_after_actor_line_emission() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Lifeline,
                        ThemeStylePatch::default()
                            .with_stroke_width(2.0)
                            .expect("valid Lifeline stroke width"),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Lifeline width theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nactor Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Sequence Lifeline width session"),
    )
    .expect("Sequence Lifeline width theme should prepare")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("every actor Lifeline width should be proven by its terminal writer");

    assert!(
        rendered
            .svg()
            .contains("#merman .actor-line{stroke-width:2px;}")
    );
    assert_eq!(rendered.svg().matches(r#"data-et="life-line""#).count(), 2);
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Lifeline,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn require_portable_accepts_sequence_lifeline_stroke_width_clear_after_baseline_emission() {
    let mut style = ThemeStylePatch::default();
    style.stroke.width = Specified::Clear;
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::Lifeline, style).for_family(DiagramFamilyId::SEQUENCE),
            )),
        )
        .expect("compile cleared Sequence Lifeline width theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nactor Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable cleared Sequence Lifeline width session"),
    )
    .expect("cleared Sequence Lifeline width theme should prepare")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("every cleared Lifeline width should be proven by its baseline terminal emission");

    assert_eq!(
        rendered
            .svg()
            .matches(r##"stroke-width="0.5px" stroke="#999""##)
            .count(),
        2,
        "Clear must restore the Mermaid baseline on actor-man and regular participant lifelines"
    );
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Lifeline,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn sequence_lifeline_stroke_wins_over_fill_independent_of_rule_order() {
    for stroke_first in [false, true] {
        let fill_rule = ThemeRule::new(
            ThemeTarget::Lifeline,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#ef4444").expect("valid Lifeline fill")),
        )
        .for_family(DiagramFamilyId::SEQUENCE);
        let stroke_rule = ThemeRule::new(
            ThemeTarget::Lifeline,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#2563eb").expect("valid Lifeline stroke")),
        )
        .for_family(DiagramFamilyId::SEQUENCE);
        let rules = if stroke_first {
            ThemeRuleSet::default()
                .with_rule(stroke_rule)
                .with_rule(fill_rule)
        } else {
            ThemeRuleSet::default()
                .with_rule(fill_rule)
                .with_rule(stroke_rule)
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile combined Sequence Lifeline theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nactor Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict combined Sequence Lifeline session"),
        )
        .expect("combined Sequence Lifeline theme should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("shadowed Lifeline fill must not leave strict evidence incomplete");

        let stroke_index = usize::from(!stroke_first);
        let fill_index = usize::from(stroke_first);
        assert!(
            rendered
                .svg()
                .contains("#merman .actor-line{stroke:#2563eb;}")
        );
        assert!(!rendered.svg().contains(".actor-line{stroke:#ef4444;}"));
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: stroke_index,
                target: ThemeTarget::Lifeline,
            }]
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: fill_index,
                target: ThemeTarget::Lifeline,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_explicit_actor_line_color_owns_precedence_over_typed_lifeline_paint() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Lifeline,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Lifeline theme");
    let cases = [
        (
            "site config",
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {"actorLineColor": "#22c55e"}
            }))),
            "sequenceDiagram\nactor Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
        ),
        (
            "init directive",
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "secure": [
                    "secure",
                    "securityLevel",
                    "startOnLoad",
                    "maxTextSize",
                    "suppressErrorRendering",
                    "maxEdges"
                ]
            }))),
            r##"%%{init: {"themeVariables": {"actorLineColor": "#22c55e"}}}%%
sequenceDiagram
actor Alice
participant Bob
Alice->>Bob: Hello
"##,
        ),
    ];

    for (case, engine, source) in cases {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("configured Sequence source should produce a render model");
        assert_eq!(
            parsed
                .metadata()
                .config
                .get_str("themeVariables.actorLineColor"),
            (case == "init directive").then_some("#22c55e"),
            "{case} source ownership probe"
        );
        assert_eq!(
            parsed
                .metadata()
                .effective_config
                .get_str("themeVariables.actorLineColor"),
            Some("#22c55e"),
            "{case} must retain explicit actorLineColor"
        );
        assert!(
            merman_core::__private::config_path_overrides_typed_default(
                &parsed.metadata().effective_config,
                "themeVariables.actorLineColor",
            ),
            "{case} must retain actorLineColor ownership"
        );
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin configured strict Sequence Lifeline session"),
        )
        .expect("explicit actorLineColor should remain evaluable")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("explicit actorLineColor should satisfy strict portability");

        assert!(
            rendered
                .svg()
                .contains("#merman .actor-line{stroke:#22c55e;}"),
            "{case}: {}",
            rendered.svg()
        );
        assert!(!rendered.svg().contains(".actor-line{stroke:#ef4444;}"));
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Lifeline,
            }]
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_unsupported_lifeline_gradient_fails_closed_after_actor_line_emission() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#000000").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#ffffff").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Lifeline,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::LinearGradient(gradient)),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile unsupported Sequence Lifeline gradient");
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nactor Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort Sequence Lifeline session"),
    )
    .expect("best-effort preparation should retain unsupported Lifeline evidence")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort Sequence Lifeline output should remain renderable");
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Lifeline,
            },
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }]
    );

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Sequence Lifeline session"),
    )
    .expect("strict verification waits for terminal Lifeline evidence");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("unsupported Sequence Lifeline gradient must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::SEQUENCE, 1))
    );
}

#[test]
fn require_portable_accepts_sequence_message_scalar_strokes_after_line_emission() {
    for (paint, expected_stroke) in [
        (
            CanvasPaint::solid("#2563eb").expect("valid Message stroke"),
            "#2563eb",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Message,
                            ThemeStylePatch::default().with_stroke(paint),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Message theme");
        let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "sequenceDiagram\nautonumber\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\nBob-->>Alice: Back\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence Message session"),
        )
        .expect("Sequence Message theme should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Sequence Message stroke should be proven by the terminal writer");

        let expected_css =
            format!("#merman .messageLine0,#merman .messageLine1{{stroke:{expected_stroke};}}");
        assert!(
            rendered.svg().contains(&expected_css),
            "Sequence SVG should contain the typed Message stroke: {}",
            rendered.svg()
        );
        assert!(
            rendered.svg().contains(r#"data-et="message""#),
            "Sequence SVG should contain terminal Message lines: {}",
            rendered.svg()
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Message,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_message_stroke_winner_accounts_for_shadowed_fill_rule() {
    for stroke_first in [false, true] {
        let fill_rule = ThemeRule::new(
            ThemeTarget::Message,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#ef4444").expect("valid Message fill")),
        )
        .for_family(DiagramFamilyId::SEQUENCE);
        let stroke_rule = ThemeRule::new(
            ThemeTarget::Message,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#2563eb").expect("valid Message stroke")),
        )
        .for_family(DiagramFamilyId::SEQUENCE);
        let rules = if stroke_first {
            ThemeRuleSet::default()
                .with_rule(stroke_rule)
                .with_rule(fill_rule)
        } else {
            ThemeRuleSet::default()
                .with_rule(fill_rule)
                .with_rule(stroke_rule)
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile combined Sequence Message theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict combined Sequence Message session"),
        )
        .expect("combined Sequence Message theme should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("shadowed Message fill must not leave strict evidence incomplete");

        let stroke_index = usize::from(!stroke_first);
        let fill_index = usize::from(stroke_first);
        assert!(
            rendered
                .svg()
                .contains("#merman .messageLine0,#merman .messageLine1{stroke:#2563eb;}")
        );
        assert!(!rendered.svg().contains("stroke:#ef4444"));
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: stroke_index,
                target: ThemeTarget::Message,
            }]
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: fill_index,
                target: ThemeTarget::Message,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_message_stroke_rule_without_a_terminal_line_is_not_applicable() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Message,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Message theme");
    for source in [
        "sequenceDiagram\nparticipant Alice\nparticipant Bob\n",
        "sequenceDiagram\nautonumber\nparticipant Alice\nparticipant Bob\nNote over Alice,Bob: Ready\n",
    ] {
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict Sequence Message session"),
        )
        .expect("Sequence Message rule without lines should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Sequence Message rule without terminal lines should be not applicable");

        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Message,
            }]
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_explicit_signal_color_owns_precedence_over_typed_message_stroke() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Message,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Message theme");
    let cases = [
        (
            "site config",
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {"signalColor": "#22c55e"}
            }))),
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
        ),
        (
            "init directive",
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "secure": [
                    "secure",
                    "securityLevel",
                    "startOnLoad",
                    "maxTextSize",
                    "suppressErrorRendering",
                    "maxEdges"
                ]
            }))),
            r##"%%{init: {"themeVariables": {"signalColor": "#22c55e"}}}%%
sequenceDiagram
participant Alice
participant Bob
Alice->>Bob: Hello
"##,
        ),
    ];

    for (case, engine, source) in cases {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("configured Sequence source should produce a render model");
        assert_eq!(
            parsed
                .metadata()
                .config
                .get_str("themeVariables.signalColor"),
            (case == "init directive").then_some("#22c55e"),
            "{case} source ownership probe"
        );
        assert_eq!(
            parsed
                .metadata()
                .effective_config
                .get_str("themeVariables.signalColor"),
            Some("#22c55e"),
            "{case} must retain explicit signalColor"
        );
        assert!(
            merman_core::__private::config_path_overrides_typed_default(
                &parsed.metadata().effective_config,
                "themeVariables.signalColor",
            ),
            "{case} must retain signalColor ownership"
        );
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin configured strict Sequence Message session"),
        )
        .expect("explicit signalColor should remain evaluable")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("explicit signalColor should satisfy strict portability");

        assert!(
            rendered.svg().contains("stroke:#22c55e"),
            "{case}: {}",
            rendered.svg()
        );
        assert!(
            !rendered
                .svg()
                .contains("#merman .messageLine0,#merman .messageLine1{stroke:#ef4444;}")
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Message,
            }]
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_text_color_derivation_owns_precedence_over_typed_message_stroke() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Message,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Message theme");
    let cases = [
        (
            "site config",
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {"textColor": "#22c55e"}
            }))),
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
        ),
        (
            "init directive",
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "secure": [
                    "secure",
                    "securityLevel",
                    "startOnLoad",
                    "maxTextSize",
                    "suppressErrorRendering",
                    "maxEdges"
                ]
            }))),
            r##"%%{init: {"themeVariables": {"textColor": "#22c55e"}}}%%
sequenceDiagram
participant Alice
participant Bob
Alice->>Bob: Hello
"##,
        ),
    ];

    for (case, engine, source) in cases {
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("configured Sequence source should produce a render model");
        assert_eq!(
            parsed.metadata().config.get_str("themeVariables.textColor"),
            (case == "init directive").then_some("#22c55e"),
            "{case} source ownership probe"
        );
        assert_eq!(
            parsed
                .metadata()
                .effective_config
                .get_str("themeVariables.textColor"),
            Some("#22c55e"),
            "{case} must retain explicit textColor"
        );
        assert_eq!(
            parsed
                .metadata()
                .effective_config
                .get_str("themeVariables.signalColor"),
            Some("#22c55e"),
            "{case} must derive signalColor from textColor"
        );
        assert!(
            merman_core::__private::config_path_overrides_typed_default(
                &parsed.metadata().effective_config,
                "themeVariables.signalColor",
            ),
            "{case} must propagate ownership to signalColor"
        );
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin configured strict Sequence Message session"),
        )
        .expect("derived signalColor should remain evaluable")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("derived signalColor should satisfy strict portability");

        assert!(
            rendered.svg().contains("stroke:#22c55e"),
            "{case}: {}",
            rendered.svg()
        );
        assert!(
            !rendered
                .svg()
                .contains("#merman .messageLine0,#merman .messageLine1{stroke:#ef4444;}")
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Message,
            }]
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_unsupported_message_gradient_fails_closed_after_line_emission() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#000000").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#ffffff").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Message,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::LinearGradient(gradient)),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile unsupported Sequence Message gradient");
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort Sequence Message session"),
    )
    .expect("best-effort preparation should retain unsupported Message evidence")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort Sequence Message output should remain renderable");
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Message,
            },
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }]
    );

    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Sequence Message session"),
    )
    .expect("strict verification waits for terminal Message evidence");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("unsupported Sequence Message gradient must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::SEQUENCE, 1))
    );
}

#[test]
fn require_portable_accepts_sequence_note_scalar_paints_after_rect_emission() {
    for (style, expected_css) in [
        (
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#ef4444").expect("valid Note fill")),
            "#merman .note{fill:#ef4444;}",
        ),
        (
            ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
            "#merman .note{fill:transparent;}",
        ),
        (
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#2563eb").expect("valid Note stroke")),
            "#merman .note{stroke:#2563eb;}",
        ),
        (
            ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent),
            "#merman .note{stroke:transparent;}",
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(ThemeTarget::Note, style).for_family(DiagramFamilyId::SEQUENCE),
                )),
            )
            .expect("compile Sequence Note theme");
        let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "sequenceDiagram\nparticipant Alice\nparticipant Bob\nNote over Alice,Bob: Ready\nAlice->>Bob: Hello\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence Note session"),
        )
        .expect("Sequence Note theme should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Sequence Note paint should be proven by the rect writer");

        assert!(
            rendered.svg().contains(expected_css),
            "Sequence SVG should contain the typed Note paint: {}",
            rendered.svg()
        );
        assert!(
            rendered.svg().contains(r#"data-et="note" data-id="i"#),
            "Sequence SVG should contain the terminal Note rect: {}",
            rendered.svg()
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Note,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_note_rule_without_a_terminal_note_rect_is_not_applicable() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Note,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").expect("valid Note fill")),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Note theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Sequence Note session"),
    )
    .expect("Sequence Note rule without notes should prepare")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("Sequence Note rule without terminal rects should be not applicable");

    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Note,
        }]
    );
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn require_portable_accepts_sequence_activation_scalar_paints_after_rect_emission() {
    for (style, expected_css) in [
        (
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#ef4444").expect("valid Activation fill")),
            "#merman .activation0,#merman .activation1,#merman .activation2{fill:#ef4444;}",
        ),
        (
            ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
            "#merman .activation0,#merman .activation1,#merman .activation2{fill:transparent;}",
        ),
        (
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#2563eb").expect("valid Activation stroke")),
            "#merman .activation0,#merman .activation1,#merman .activation2{stroke:#2563eb;}",
        ),
        (
            ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent),
            "#merman .activation0,#merman .activation1,#merman .activation2{stroke:transparent;}",
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(ThemeTarget::Activation, style)
                            .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence Activation theme");
        let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Request\nactivate Bob\nBob-->>Alice: Response\ndeactivate Bob\nAlice->>Bob: Pending\nactivate Bob\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence Activation session"),
        )
        .expect("Sequence Activation theme should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Sequence Activation paint should be proven by the rect writer");

        assert!(
            rendered.svg().contains(expected_css),
            "Sequence SVG should contain the typed Activation paint: {}",
            rendered.svg()
        );
        assert!(
            rendered.svg().contains(r#"class="activation0""#),
            "Sequence SVG should contain the terminal Activation rect: {}",
            rendered.svg()
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Activation,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_activation_rule_without_a_terminal_rect_is_not_applicable() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Activation,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#ef4444").expect("valid Activation fill"),
                        ),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Activation theme");
    for source in [
        "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
        "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Pending\nactivate Bob\n",
    ] {
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict Sequence Activation session"),
        )
        .expect("Sequence Activation rule without terminal rectangles should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Sequence Activation rule without terminal rectangles should be inapplicable");

        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Activation,
            }]
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_explicit_activation_paints_own_precedence_over_typed_paints() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Activation,
                        ThemeStylePatch::default()
                            .with_fill(
                                CanvasPaint::solid("#ef4444").expect("valid Activation fill"),
                            )
                            .with_stroke(
                                CanvasPaint::solid("#2563eb").expect("valid Activation stroke"),
                            ),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Activation theme");
    let parsed = theme
            .install_parse_compatibility(Engine::new().with_site_config(
                MermaidConfig::from_value(json!({
                    "themeVariables": {
                        "activationBkgColor": "#22c55e",
                        "activationBorderColor": "#a855f7"
                    }
                })),
            ))
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Request\nactivate Bob\nBob-->>Alice: Response\ndeactivate Bob\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict configured Sequence Activation session"),
    )
    .expect("explicit Mermaid Activation paints should remain evaluable")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("explicit Mermaid Activation paints should satisfy strict portability");

    assert!(rendered.svg().contains("#22c55e"), "{}", rendered.svg());
    assert!(rendered.svg().contains("#a855f7"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("#ef4444"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("#2563eb"), "{}", rendered.svg());
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Activation,
        }]
    );
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn sequence_explicit_note_paints_own_precedence_over_typed_note_paints() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Note,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").expect("valid Note fill"))
                            .with_stroke(CanvasPaint::solid("#2563eb").expect("valid Note stroke")),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Note theme");
    let parsed = theme
            .install_parse_compatibility(Engine::new().with_site_config(
                MermaidConfig::from_value(json!({
                    "themeVariables": {
                        "noteBkgColor": "#22c55e",
                        "noteBorderColor": "#a855f7"
                    }
                })),
            ))
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nNote over Alice,Bob: Ready\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict configured Sequence Note session"),
    )
    .expect("explicit Mermaid Note paints should remain evaluable")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("explicit Mermaid Note paints should satisfy strict portability");

    assert!(rendered.svg().contains("#22c55e"), "{}", rendered.svg());
    assert!(rendered.svg().contains("#a855f7"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("#ef4444"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("#2563eb"), "{}", rendered.svg());
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Note,
        }]
    );
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn sequence_actor_stroke_explicit_mermaid_border_owns_precedence() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor stroke theme");
    let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
        MermaidConfig::from_value(json!({
            "themeVariables": {"actorBorder": "#22c55e"}
        })),
    ));
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Sequence session"),
    )
    .expect("explicit Mermaid actor stroke should remain evaluable")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("explicit Mermaid actor stroke should satisfy strict portability");

    assert!(rendered.svg().contains("#22c55e"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("#2563eb"), "{}", rendered.svg());
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn sequence_actor_stroke_with_unhandled_shapes_fails_closed() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor stroke theme");

    for source in [
        "sequenceDiagram\nparticipant Alice@{\"type\":\"control\"}\nparticipant Bob\nAlice->>Bob: Hello\n",
        "sequenceDiagram\nparticipant Alice@{\"type\":\"database\"}\nparticipant Bob\nAlice->>Bob: Hello\n",
        "sequenceDiagram\nparticipant Alice\nparticipant Bob\nproperties Alice: {\"class\":\"custom\"}\nAlice->>Bob: Hello\n",
    ] {
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Sequence source should produce a render model");
        let strict = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Sequence session");
        let artifact = prepare(parsed, &LayoutOptions::default(), strict)
            .expect("strict verification must wait for terminal SVG evidence");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unhandled actor stroke shape must not be signed as portable"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::SEQUENCE, 1))
        );
    }
}

#[test]
fn sequence_actor_stroke_tracks_every_css_covered_shape() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor stroke theme");

    for actor_type in ["actor", "boundary", "entity", "collections", "queue"] {
        let source = format!(
            "sequenceDiagram\nparticipant Alice@{{\"type\":\"{actor_type}\"}}\nparticipant Bob\nAlice->>Bob: Hello\n"
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence session"),
        )
        .expect("strict verification must wait for terminal SVG evidence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap_or_else(|error| {
            panic!("{actor_type} should consume the typed actor stroke: {error}")
        });

        assert!(
            rendered.svg().contains("stroke:#2563eb"),
            "{actor_type} SVG should contain the typed actor stroke: {}",
            rendered.svg()
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }],
            "{actor_type} should produce writer-owned Actor evidence"
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }
}

#[test]
fn sequence_actor_stroke_transparent_paint_is_signed_only_for_covered_shapes() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile transparent Sequence actor stroke theme");
    let parse = |source: &str| {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Sequence source should produce a render model")
    };
    let strict = || {
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Sequence session")
    };

    let rendered = prepare(
            parse(
                "sequenceDiagram\nparticipant Alice@{\"type\":\"queue\"}\nparticipant Bob\nAlice->>Bob: Hello\n",
            ),
            &LayoutOptions::default(),
            strict(),
        )
        .expect("transparent actor stroke should wait for terminal SVG evidence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("queue should consume the transparent typed actor stroke");
    assert!(rendered.svg().contains("stroke:transparent"));
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());

    let artifact = prepare(
            parse(
                "sequenceDiagram\nparticipant Alice@{\"type\":\"control\"}\nparticipant Bob\nAlice->>Bob: Hello\n",
            ),
            &LayoutOptions::default(),
            strict(),
        )
        .expect("control stroke verification must wait for terminal SVG evidence");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("control marker must not be signed as transparent typed stroke"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::SEQUENCE, 1))
    );
}

#[test]
fn sequence_actor_stroke_does_not_recolor_autonumber_carrier_or_message_lines() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor stroke theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nautonumber\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("autonumbered Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Sequence session"),
    )
    .expect("actor stroke should wait for terminal SVG evidence")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("autonumbered Sequence should retain isolated Actor stroke evidence");
    let svg = rendered.svg();

    assert!(svg.contains("stroke:#2563eb"), "{svg}");
    assert!(
        svg.contains(r#"class="messageLine0""#),
        "fixture must contain a non-Actor message line: {svg}"
    );
    assert!(
        svg.contains(r#"stroke-width="0" marker-start="url(#merman-sequencenumber)""#),
        "fixture must contain the unclassified autonumber carrier: {svg}"
    );
    let typed_rules_start = svg
        .find("#merman .actor{stroke:#2563eb;}")
        .expect("typed Actor stroke rule should be emitted");
    let typed_rules_end = svg[typed_rules_start..]
        .find("#merman g rect.rect")
        .map(|offset| typed_rules_start + offset)
        .expect("typed Actor stroke rules should precede the next legacy rule");
    let typed_rules = &svg[typed_rules_start..typed_rules_end];
    assert!(!typed_rules.contains("#merman line"), "{typed_rules}");
    assert!(!typed_rules.contains("messageLine"), "{typed_rules}");
    assert!(!typed_rules.contains("sequencenumber"), "{typed_rules}");
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
}

#[test]
fn sequence_explicit_actor_fill_owns_precedence_over_typed_actor_fill() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor theme");
    let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
        MermaidConfig::from_value(json!({
            "themeVariables": {"actorBkg": "#22c55e"}
        })),
    ));
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable Sequence session"),
    )
    .expect("explicit Mermaid actor fill should remain evaluable")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("explicit Mermaid actor fill should satisfy strict portability");

    assert!(rendered.svg().contains("#22c55e"), "{}", rendered.svg());
    assert!(!rendered.svg().contains("#ef4444"), "{}", rendered.svg());
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn sequence_unsupported_actor_gradient_fails_closed_after_svg_emission() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#000000").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#ffffff").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile unsupported Sequence actor gradient");
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort Sequence session"),
    )
    .expect("best-effort preparation should retain unsupported evidence")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort Sequence output should remain renderable");
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            },
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }]
    );

    let strict = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict Sequence session");
    let artifact = prepare(parse(), &LayoutOptions::default(), strict)
        .expect("strict verification waits for terminal SVG evidence");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("unsupported Sequence actor gradient must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::SEQUENCE, 1))
    );

    let configured_parse = || {
        theme
            .install_parse_compatibility(Engine::new().with_site_config(MermaidConfig::from_value(
                json!({
                    "themeVariables": {"actorBkg": "#22c55e"}
                }),
            )))
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("configured Sequence source should produce a render model")
    };
    let configured = prepare(
        configured_parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin configured strict Sequence session"),
    )
    .expect("explicit actor fill should suppress the unsupported theme gradient")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("explicit actor fill should satisfy strict portability");
    assert!(configured.svg().contains("#22c55e"), "{}", configured.svg());
    assert!(configured.style_report().theme_residuals().is_empty());
    assert_eq!(
        configured.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
}

#[test]
fn sequence_actor_fill_with_custom_class_does_not_claim_typed_consumption() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor theme");
    let source = r#"sequenceDiagram
participant Alice
participant Bob
properties Alice: {"class":"custom"}
Alice->>Bob: Hello
"#;
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Sequence source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort Sequence session"),
    )
    .expect("prepare Sequence actor theme")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort Sequence output should remain renderable");

    assert!(
        rendered.svg().contains("fill:#ef4444"),
        "{}",
        rendered.svg()
    );
    assert!(
        rendered.svg().contains("fill=\"#EDF2AE\""),
        "custom actor fill must remain source-owned: {}",
        rendered.svg()
    );
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            },
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }]
    );
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );

    let strict = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict Sequence session");
    let artifact = prepare(parse(), &LayoutOptions::default(), strict)
        .expect("strict verification must wait for terminal SVG evidence");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("custom actor class must not be signed as typed portable fill"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::SEQUENCE, 1))
    );
}

#[test]
fn sequence_actor_fill_tracks_actor_man_css_and_inline_precedence() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence actor theme");

    for (actor_type, should_be_portable) in [
        ("actor", true),
        ("boundary", true),
        ("entity", true),
        ("collections", true),
        ("queue", true),
        ("database", true),
        ("control", false),
    ] {
        let source = format!(
            "sequenceDiagram\nparticipant Alice@{{\"type\":\"{actor_type}\"}}\nparticipant Bob\nAlice->>Bob: Hello\n"
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .expect("Sequence source should produce a render model");
        let strict = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Sequence session");
        let artifact = prepare(parsed, &LayoutOptions::default(), strict)
            .expect("strict verification must wait for terminal SVG evidence");
        let result = artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());

        if should_be_portable {
            let rendered = result.expect("actor-man CSS-covered fill should be portable");
            assert!(rendered.svg().contains("fill:#ef4444"));
            assert_eq!(
                rendered.style_report().theme_applied_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Actor,
                }]
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        } else {
            let error = match result {
                Ok(_) => panic!("control marker fill must not be signed as typed"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::SEQUENCE, 1))
            );
        }
    }
}

#[test]
fn sequence_actor_fill_accounts_for_rules_without_matching_actors() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                    )
                    .with_ordinal(OrdinalSelector::exact(3).unwrap())
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile unmatched ordinal Sequence actor theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Sequence session"),
    )
    .expect("prepare unmatched ordinal Sequence actor rule")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("unmatched ordinal Sequence actor rule should be not applicable");

    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
}

#[test]
fn sequence_actor_ordinal_evidence_charges_once_per_actual_actor() {
    let ordinal = OrdinalSelector::cycle(1, 0).unwrap();
    let mut radius = ThemeStylePatch::default();
    radius.geometry.radius = Specified::Value(6.0);
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .with_ordinal(ordinal)
                        .for_family(DiagramFamilyId::SEQUENCE),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .with_ordinal(ordinal)
                        .for_family(DiagramFamilyId::SEQUENCE),
                    )
                    .with_rule(
                        ThemeRule::new(ThemeTarget::Actor, radius)
                            .with_ordinal(ordinal)
                            .for_family(DiagramFamilyId::SEQUENCE),
                    ),
            ),
        )
        .expect("compile ordinal Sequence actor theme");
    let source = "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n";
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Sequence source should produce a render model")
    };
    let environment_with_limit = |limit| {
        crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            )
            .begin_session_with_theme(&theme)
            .expect("begin bounded Sequence session")
    };

    let unbounded = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin unbounded Sequence session"),
    )
    .expect("prepare unbounded Sequence artifact");
    let layout_work_units = unbounded.context.session.work_meter().used();
    drop(unbounded);

    // Three ordinal candidates are resolved once for each of the two actual actors. The
    // three facets must share those two resolutions instead of each rescanning both actors.
    let evidence_work_units = 3 * 2;
    let exact_limit = layout_work_units + evidence_work_units;
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        environment_with_limit(exact_limit),
    )
    .expect("exact limit must admit Sequence layout")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("one ordinal resolution per actual actor must fit the exact limit");
    assert_eq!(rendered.session.work_meter().used(), exact_limit);
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[
            FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Actor,
                },
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            },
            FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::Actor,
                },
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            },
            FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 2,
                    target: ThemeTarget::Actor,
                },
                reason: FamilyThemeResidualReason::UnsupportedGeometry,
            },
        ]
    );

    let short_limit = exact_limit - 1;
    let artifact = prepare(
        parse(),
        &LayoutOptions::default(),
        environment_with_limit(short_limit),
    )
    .expect("short limit must still admit Sequence layout");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("ordinal evidence must fail closed when its work exceeds the limit"),
        Err(error) => error,
    };
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_layout_work_units resource rejection")
    };
    assert_eq!(limit.limit, "max_layout_work_units");
    assert_eq!(limit.max, short_limit);
    assert_eq!(limit.actual, exact_limit);
}

#[test]
fn sequence_actor_fill_winner_accounts_for_superseded_and_unsupported_facets() {
    let mut winning_style =
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2563eb").unwrap());
    winning_style.geometry.radius = Specified::Value(6.0);
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    )
                    .with_rule(
                        ThemeRule::new(ThemeTarget::Actor, winning_style)
                            .for_family(DiagramFamilyId::SEQUENCE),
                    ),
            ),
        )
        .expect("compile competing Sequence actor rules");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Sequence source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort Sequence session"),
    )
    .expect("prepare competing Sequence actor rules")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render competing Sequence actor rules");

    assert!(rendered.svg().contains("fill:#2563eb"));
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .is_empty()
    );
    assert_eq!(
        rendered.style_report().theme_not_applicable_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        }]
    );
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::Actor,
            },
            reason: FamilyThemeResidualReason::UnsupportedGeometry,
        }]
    );
}

#[test]
fn state_style_residual_survives_terminal_svg_completion() {
    let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nclassDef broken font-size:not-a-size\n[*] --> Ready:::broken\nReady --> [*]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
    let rendered = prepare(parsed, &LayoutOptions::default(), session())
        .expect("best-effort State preparation")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State SVG");

    assert_eq!(rendered.family_id(), DiagramFamilyId::STATE);
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Unverified
    );
    let residual = rendered
        .style_report()
        .residuals()
        .first()
        .cloned()
        .expect("invalid font-size residual");
    assert_eq!(residual.property(), Some("font-size"));
    assert_eq!(residual.owner_id(), "Ready");
    assert_eq!(residual.class_id(), Some("broken"));
    assert_eq!(residual.origin(), FamilyStyleOrigin::AssignedClass);
    assert_eq!(residual.channel(), FamilyStyleChannel::Label);
    assert_eq!(residual.reason(), FamilyStyleResidualReason::InvalidValue);

    let completion = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("finalize State SVG")
        .into_completion();
    assert_eq!(
        completion.report().style_report().residuals(),
        std::slice::from_ref(&residual)
    );
}

#[test]
fn state_structured_gradient_is_a_semantic_residual_not_verified_source_css() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#0f172a").expect("valid first stop"),
            )
            .expect("valid first stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#22d3ee").expect("valid second stop"),
            )
            .expect("valid second stop"),
        ],
    )
    .expect("valid gradient");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::State,
                ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
            ))),
        )
        .expect("compile gradient theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed session"),
    )
    .expect("best-effort State preparation")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render State SVG");

    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Unverified
    );
    let residual = rendered
        .style_report()
        .theme_residuals()
        .first()
        .expect("gradient must retain semantic residual");
    assert_eq!(
        residual.reason(),
        FamilyThemeResidualReason::UnsupportedPaint
    );
    assert!(rendered.style_report().residuals().is_empty());
}

#[test]
fn state_ordinal_palette_is_applied_by_an_observed_state_binding() {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#0f172a").expect("valid first palette color"),
        ThemeColorValue::parse("#22d3ee").expect("valid second palette color"),
    ])
    .expect("valid ordinal palette");
    let theme =
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::State, palette),
            ))
            .expect("compile ordinal State theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed session"),
    )
    .expect("best-effort State preparation")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render State SVG");

    let key = FamilyThemeMechanismKey::OrdinalPalette {
        target: ThemeTarget::State,
    };
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .contains(&key)
    );
    assert!(
        !rendered
            .style_report()
            .theme_residuals()
            .iter()
            .any(|residual| residual.key() == &key)
    );
    assert!(rendered.svg().contains("#0f172a"));
}

#[test]
fn state_debug_visibility_filters_downgrade_applied_theme_evidence() {
    let node_theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#ef4444").expect("valid State fill")),
                )),
            ),
        )
        .expect("compile State node theme");
    let node_parsed = node_theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\nReady --> Done\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let mut node_debug = SvgDebugOptions::default();
    node_debug.include_nodes = false;
    let node_rendered = prepare(
        node_parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&node_theme)
            .expect("begin themed session"),
    )
    .expect("prepare State node theme")
    .render_svg(&SvgRenderOptions::default(), &node_debug)
    .expect("render filtered State SVG");

    let node_key = FamilyThemeMechanismKey::Rule {
        index: 0,
        target: ThemeTarget::State,
    };
    assert!(!node_rendered.svg().contains("#ef4444"));
    assert_eq!(
        node_rendered.style_report().verification(),
        FamilyStyleVerification::Unverified
    );
    assert!(
        !node_rendered
            .style_report()
            .theme_applied_mechanisms()
            .contains(&node_key)
    );
    assert!(
        node_rendered
            .style_report()
            .theme_residuals()
            .iter()
            .any(|residual| {
                residual.key() == &node_key
                    && residual.reason() == FamilyThemeResidualReason::OutputVisibilityFiltered
            })
    );

    let edge_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Transition,
                    ThemeStylePatch::default().with_stroke(
                        CanvasPaint::solid("#ec4899").expect("valid transition stroke"),
                    ),
                ),
            ),
        ))
        .expect("compile State transition theme");
    let edge_parsed = edge_theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\nReady --> Done\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let mut edge_debug = SvgDebugOptions::default();
    edge_debug.include_edges = false;
    let edge_rendered = prepare(
        edge_parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&edge_theme)
            .expect("begin themed session"),
    )
    .expect("prepare State transition theme")
    .render_svg(&SvgRenderOptions::default(), &edge_debug)
    .expect("render filtered State SVG");

    let edge_key = FamilyThemeMechanismKey::Rule {
        index: 0,
        target: ThemeTarget::Transition,
    };
    assert!(!edge_rendered.svg().contains("#ec4899"));
    assert!(
        edge_rendered
            .style_report()
            .theme_residuals()
            .iter()
            .any(|residual| {
                residual.key() == &edge_key
                    && residual.reason() == FamilyThemeResidualReason::OutputVisibilityFiltered
            })
    );
}

#[test]
fn require_portable_rechecks_state_theme_after_debug_visibility_filter() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::State,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#ef4444").expect("valid State fill")),
                )),
            ),
        )
        .expect("compile strict State theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\nReady --> Done\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let artifact = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict themed session"),
    )
    .expect("complete strict State preparation before output filtering");
    let mut debug = SvgDebugOptions::default();
    debug.include_nodes = false;

    let error = match artifact.render_svg(&SvgRenderOptions::default(), &debug) {
        Ok(_) => panic!("filtered State output must not retain portable family evidence"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::UnverifiedFamilyTheme {
            family_id: DiagramFamilyId::STATE,
            residual_count: 1,
        }
    ));
}

#[test]
fn state_family_typography_is_shared_by_layout_and_terminal_svg() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(26.0)
        .expect("valid State font size");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_default(typography)),
        )
        .expect("compile State typography theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "---\ntitle: Architecture\n---\nstateDiagram-v2\n[*] --> Ready\nReady --> Done\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed session"),
    )
    .expect("State typography preparation")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render State SVG");

    assert!(
        rendered
            .style_report()
            .theme_applied_mechanisms()
            .contains(&FamilyThemeMechanismKey::Typography)
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
    assert!(rendered.svg().contains("font-size:26px"));
    assert!(
        rendered.svg().contains("font-size:26px !important"),
        "node/title inline emission must match the measured typography: {}",
        rendered.svg()
    );
    assert!(
        rendered
            .svg()
            .contains("statediagramTitleText{text-anchor:middle;font-family:"),
        "title CSS must use the computed State text style: {}",
        rendered.svg()
    );
}

#[test]
fn state_spacing_without_prepared_text_is_suppressed_and_reported() {
    let typography = ThemeTextStyle::default()
        .with_letter_spacing_px(10.0)
        .expect("valid State letter spacing")
        .with_word_spacing_px(6.0)
        .expect("valid State word spacing");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_default(typography)),
        )
        .expect("compile State spacing theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\nReady --> Done\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed session without a custom font catalog"),
    )
    .expect("best-effort State spacing preparation")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render State SVG");

    assert!(!rendered.svg().contains("letter-spacing"));
    assert!(!rendered.svg().contains("word-spacing"));
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Unverified
    );
    assert!(
        rendered
            .style_report()
            .theme_residuals()
            .iter()
            .any(|residual| {
                residual.key() == &FamilyThemeMechanismKey::Typography
                    && residual.reason() == FamilyThemeResidualReason::UnsupportedTypography
            })
    );
}

#[test]
fn state_rule_without_a_matching_document_target_is_not_applicable() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Note,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#fef3c7").expect("valid note fill")),
                )),
            ),
        )
        .expect("compile Note theme");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let rendered = prepare(
        parsed,
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed session"),
    )
    .expect("best-effort State preparation")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render State SVG");

    let key = FamilyThemeMechanismKey::Rule {
        index: 0,
        target: ThemeTarget::Note,
    };
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
    assert!(
        rendered
            .style_report()
            .theme_not_applicable_mechanisms()
            .contains(&key)
    );
    assert!(
        !rendered
            .style_report()
            .theme_applied_mechanisms()
            .contains(&key)
    );
    assert!(rendered.style_report().theme_residuals().is_empty());
}

#[test]
fn require_portable_rejects_state_semantic_residual_before_layout() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#0f172a").expect("valid first stop"),
            )
            .expect("valid first stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#22d3ee").expect("valid second stop"),
            )
            .expect("valid second stop"),
        ],
    )
    .expect("valid gradient");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::State,
                ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
            ))),
        )
        .expect("compile gradient theme");
    let parse = || {
        theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort State session"),
    )
    .expect("prepare State semantic residual")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort State output should retain semantic residual evidence");
    assert_eq!(
        rendered.style_report().theme_residuals(),
        &[FamilyThemeResidual {
            key: FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::State,
            },
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }]
    );

    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable render session");

    let error = match prepare(parse(), &LayoutOptions::default(), session) {
        Ok(_) => panic!("strict portability must reject a State semantic residual"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((
            DiagramFamilyId::STATE,
            rendered.style_report().theme_residuals().len(),
        ))
    );
}

#[test]
fn require_portable_rejects_state_style_residual_before_layout() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile portable theme recipe");
    let parse = || {
        theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "stateDiagram-v2\nclassDef broken font-size:not-a-size\n[*] --> Ready:::broken\nReady --> [*]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("State source should produce a render model")
    };
    let rendered = prepare(
        parse(),
        &LayoutOptions::default(),
        crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort State session"),
    )
    .expect("prepare State source-style residual")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort State output should retain source-style residual evidence");
    assert_eq!(rendered.style_report().residuals().len(), 1);
    let residual = &rendered.style_report().residuals()[0];
    assert_eq!(residual.owner_id(), "Ready");
    assert_eq!(residual.property(), Some("font-size"));
    assert_eq!(residual.reason(), FamilyStyleResidualReason::InvalidValue);

    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict portable render session");

    let error = match prepare(parse(), &LayoutOptions::default(), session) {
        Ok(_) => panic!("strict portability must reject an unverified State style"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_style(),
        Some((
            DiagramFamilyId::STATE,
            rendered.style_report().residuals().len(),
        ))
    );
}

#[test]
fn parsed_theme_binding_must_match_render_session_theme() {
    let compiler = DiagramThemeCompiler::new();
    let parsed_theme = compiler
        .compile_preset(crate::diagram_theme::ThemePreset::EditorDark)
        .expect("parse theme should compile");
    let render_theme = compiler
        .compile_preset(crate::diagram_theme::ThemePreset::EditorLight)
        .expect("render theme should compile");
    let parsed = parsed_theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\n[*] --> Ready\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("State source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&render_theme)
        .expect("render session should start");

    let error = match prepare(parsed, &LayoutOptions::default(), session) {
        Ok(_) => panic!("a parsed artifact must not cross theme sessions"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::ThemeParseBindingMismatch));
}

#[test]
fn recipe_identity_cannot_be_paired_with_a_different_compatibility_config() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(crate::diagram_theme::ThemePreset::EditorDark)
        .expect("theme should compile");
    for compatibility in [
        merman_core::MermaidConfig::empty_object(),
        merman_core::MermaidConfig::from_value(json!({
            "theme": "base",
            "darkMode": false,
            "themeVariables": {"darkMode": false}
        })),
    ] {
        let plan = ThemeCompatibilityPlan::try_new(
            *theme.recipe_fingerprint().as_bytes(),
            compatibility,
            |_family, _control| Ok(None),
        )
        .expect("bounded compatibility config");
        let parsed = install_theme_compatibility(Engine::new(), &plan)
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("render session should start");

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Ok(_) => panic!("compatibility config is part of the parse binding"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::ThemeParseBindingMismatch));
    }
}

#[test]
fn themed_and_unthemed_parse_session_transitions_are_fail_closed() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(crate::diagram_theme::ThemePreset::EditorDark)
        .expect("theme should compile");

    let themed_parse = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\n[*] --> Ready\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("themed State source should produce a render model");
    let unthemed_session = crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("unthemed render session should start");
    assert!(matches!(
        prepare(themed_parse, &LayoutOptions::default(), unthemed_session),
        Err(Error::ThemeParseBindingMismatch)
    ));

    let unthemed_parse = Engine::new()
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\n[*] --> Ready\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("unthemed State source should produce a render model");
    let themed_session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("themed render session should start");
    assert!(matches!(
        prepare(unthemed_parse, &LayoutOptions::default(), themed_session),
        Err(Error::ThemeParseBindingMismatch)
    ));

    let unthemed_parse = Engine::new()
        .parse_diagram_for_render_model_sync(
            "stateDiagram-v2\n[*] --> Ready\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("unthemed State source should produce a render model");
    let unthemed_session = crate::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("unthemed render session should start");
    prepare(unthemed_parse, &LayoutOptions::default(), unthemed_session)
        .expect("unthemed parses remain valid in unthemed sessions");
}

#[test]
fn equivalent_theme_instances_retain_selected_family_typed_evidence() {
    let spec = DiagramThemeSpec::new().with_styles(
        ThemeRuleSet::default().with_rule(
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#ef4444").expect("valid node fill")),
            )
            .for_family(DiagramFamilyId::FLOWCHART),
        ),
    );
    let compiler = DiagramThemeCompiler::new();
    let parse_theme = compiler
        .compile(spec.clone())
        .expect("parse theme should compile");
    let render_theme = compiler
        .compile(spec)
        .expect("equivalent render theme should compile");
    assert_eq!(
        parse_theme.recipe_fingerprint(),
        render_theme.recipe_fingerprint()
    );

    let parsed = parse_theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&render_theme)
        .expect("equivalent render session should start");

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .expect("equivalent theme should retain the typed route")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("equivalent theme should retain emitted typed evidence");
    assert!(rendered.svg().contains("fill:#ef4444 !important"));
    assert_eq!(
        rendered.style_report().verification(),
        FamilyStyleVerification::Verified
    );
    assert_eq!(
        rendered.style_report().theme_applied_mechanisms(),
        &[FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }]
    );
}

#[test]
fn planned_family_drives_flowchart_router_before_layout() {
    let engine = Engine::new();
    let options = LayoutOptions::default();

    let flowchart = engine
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model");
    let swimlane = super::preparation::prepare_non_class_render(
        flowchart,
        &options,
        FamilyRenderContext::resolve(session(), DiagramFamilyId::SWIMLANE),
    )
    .expect("planned Swimlane family should drive layout");
    assert_eq!(swimlane.family_id(), DiagramFamilyId::SWIMLANE);

    let configured_swimlane = engine
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA --> B\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("configured Swimlane source should produce a render model");
    let flowchart = super::preparation::prepare_non_class_render(
        configured_swimlane,
        &options,
        FamilyRenderContext::resolve(session(), DiagramFamilyId::FLOWCHART),
    )
    .expect("planned Flowchart family should drive layout");
    assert_eq!(flowchart.family_id(), DiagramFamilyId::FLOWCHART);
}

#[test]
fn flowchart_router_rejects_an_incompatible_planned_family() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model");

    let error = match super::preparation::prepare_non_class_render(
        parsed,
        &LayoutOptions::default(),
        FamilyRenderContext::resolve(session(), DiagramFamilyId::STATE),
    ) {
        Ok(_) => panic!("State cannot consume a Flowchart semantic model"),
        Err(error) => error,
    };
    let Error::InvalidModel { message } = error else {
        panic!("expected invalid model error")
    };
    assert!(message.contains("state"));
    assert!(message.contains("Flowchart"));
}

#[test]
fn family_artifact_rejects_planned_family_drift() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model");
    let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
    let FamilyRenderArtifact {
        metadata, family, ..
    } = artifact;

    let error = match FamilyRenderArtifact::new(
        metadata,
        family,
        FamilyRenderContext::resolve(session(), DiagramFamilyId::STATE),
    ) {
        Ok(_) => panic!("family drift must be rejected"),
        Err(error) => error,
    };
    let Error::InvalidModel { message } = error else {
        panic!("expected invalid model error")
    };
    assert!(message.contains("state"));
    assert!(message.contains("flowchart"));
}

#[test]
fn unthemed_family_completion_has_no_theme_identity() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model");
    let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
    assert!(artifact.context.resolved_theme().is_none());

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render unthemed family SVG");
    assert_eq!(rendered.family_id(), DiagramFamilyId::FLOWCHART);

    let completion = rendered.into_completion();
    assert_eq!(completion.report().family_id(), DiagramFamilyId::FLOWCHART);
    assert_eq!(completion.report().theme_recipe_fingerprint(), None);
    assert_eq!(
        completion
            .report()
            .session_report()
            .theme_recipe_fingerprint(),
        None
    );
}

fn text_measurement_call_count(session: &RenderSession) -> u64 {
    session
        .text_measurement_report()
        .entries()
        .iter()
        .map(crate::environment::TextMeasurementSummary::count)
        .sum()
}

#[derive(Debug, Clone, Copy)]
enum SidecarHostOutcome {
    Success,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SidecarHostRequest {
    ordinal: usize,
    phase: TextMeasurementPhase,
    operation: TextMeasurementOperation,
    result_kind: TextMeasurementResultKind,
    text: String,
    font_size_bits: u64,
    max_width_bits: Option<u64>,
    wrap_mode: WrapMode,
}

struct SidecarRecordingHost {
    outcome: SidecarHostOutcome,
    requests: Mutex<Vec<SidecarHostRequest>>,
}

impl SidecarRecordingHost {
    fn new(outcome: SidecarHostOutcome) -> Self {
        Self {
            outcome,
            requests: Mutex::new(Vec::new()),
        }
    }

    fn snapshot(&self) -> Vec<SidecarHostRequest> {
        self.requests.lock().expect("host request trace").clone()
    }
}

impl HostTextMeasurer for SidecarRecordingHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let ordinal = {
            let mut requests = self.requests.lock().expect("host request trace");
            let ordinal = requests.len();
            requests.push(SidecarHostRequest {
                ordinal,
                phase: request.phase,
                operation: request.operation,
                result_kind: request.operation.required_result_kind(),
                text: request.text.to_string(),
                font_size_bits: request.style.font_size.to_bits(),
                max_width_bits: request.max_width.map(f64::to_bits),
                wrap_mode: request.wrap_mode,
            });
            ordinal
        };

        match self.outcome {
            SidecarHostOutcome::Success => Ok(Some(sidecar_host_measurement(request, ordinal))),
        }
    }
}

fn sidecar_host_measurement(
    request: HostTextMeasurementRequest<'_>,
    ordinal: usize,
) -> HostTextMeasurement {
    let state_delta = (ordinal % 7) as f64 / 32.0;
    let raw_width = request
        .text
        .lines()
        .map(|line| line.chars().count() as f64 * 8.0)
        .fold(0.0_f64, f64::max)
        + state_delta;
    let max_width = request
        .max_width
        .filter(|width| width.is_finite() && *width > 0.0);
    let line_count = max_width
        .map(|width| (raw_width / width).ceil() as usize)
        .unwrap_or(1)
        .max(request.text.lines().count())
        .max(1)
        .min(request.text.len().saturating_add(1));
    let metrics = TextMetrics {
        width: max_width.map_or(raw_width, |width| raw_width.min(width)),
        height: line_count as f64 * 20.0 + state_delta,
        line_count,
    };

    match request.operation.required_result_kind() {
        TextMeasurementResultKind::Metrics => HostTextMeasurement::Metrics(metrics),
        TextMeasurementResultKind::Length => {
            let length = match request.operation {
                TextMeasurementOperation::RawBBoxHeight
                | TextMeasurementOperation::SimpleBBoxHeight
                | TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                TextMeasurementOperation::CreateTextBBoxYOffset
                | TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                _ => raw_width,
            };
            HostTextMeasurement::Length(length)
        }
        TextMeasurementResultKind::HorizontalExtents => HostTextMeasurement::HorizontalExtents {
            left: raw_width / 2.0,
            right: raw_width / 2.0,
        },
        TextMeasurementResultKind::WrappedWithRawWidth => {
            HostTextMeasurement::WrappedWithRawWidth {
                metrics,
                raw_width: Some(raw_width),
            }
        }
    }
}

#[cfg(feature = "layout-cytoscape")]
fn prepare_mindmap_with_host_limit(
    max_layout_work_units: Option<usize>,
) -> (Result<FamilyRenderArtifact>, Arc<SidecarRecordingHost>) {
    let source = "mindmap\n  Root\n    First child\n    Second child\n";
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse mindmap")
        .expect("detect mindmap");
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.mindmap-cose-budget").expect("profile id"),
        "1",
    )
    .expect("profile identity");
    let host = Arc::new(SidecarRecordingHost::new(SidecarHostOutcome::Success));
    let mut environment =
        crate::environment::RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::host_display(identity, host.clone(), TextMeasurementPhase::ALL),
        );
    if let Some(limit) = max_layout_work_units {
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, limit)
            .expect("layout work limit");
        environment = environment.with_resource_policy(policy);
    }
    let session = environment.begin_session().expect("render session");
    (prepare(parsed, &LayoutOptions::default(), session), host)
}

#[test]
fn public_flowchart_preparation_enables_prepared_svg_label_reuse() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A -->|control label| B
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("detect flowchart");
    let artifact = prepare(parsed, &LayoutOptions::default(), session())
        .expect("prepare public flowchart artifact");
    let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
        panic!("expected Flowchart family artifact");
    };

    assert!(
        flowchart
            .svg_label_sidecar()
            .node_owner("A", false)
            .is_some(),
        "the public Flowchart preparation path must build the label sidecar"
    );
}

#[test]
fn prepared_self_loop_edge_label_keeps_its_semantic_owner_through_family_dispatch() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A ordinary-edge@-->|ordinary owner sentinel| B
A self-loop-edge@-->|self loop semantic owner keeps wrapped label rows through the logical render id alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron| A
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("detect flowchart");
    let artifact = prepare(parsed, &LayoutOptions::default(), session())
        .expect("prepare flowchart family artifact");

    let rendered_svg = {
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            panic!("expected Flowchart family artifact");
        };
        let model = crate::flowchart::FlowchartRenderModelRef::new(
            flowchart.pair().semantic(),
            flowchart.label_sources(),
        );
        let edge = model.edges.get(1).expect("self-loop edge");
        assert_eq!(edge.id, "self-loop-edge");
        let label = model
            .edge_label_for_render(1, edge)
            .expect("self-loop edge label");
        let owner = flowchart
            .svg_label_sidecar()
            .edge_owner(edge.id.as_str(), false)
            .expect("semantic self-loop owner");
        assert_eq!(owner, crate::flowchart::FlowchartSvgLabelOwner::Edge(1));
        assert_eq!(
            flowchart
                .svg_label_sidecar()
                .edge_owner("A-cyclic-special-mid", false),
            None
        );
        assert!(
            flowchart
                .pair()
                .layout()
                .edges
                .iter()
                .any(|edge| edge.id == "self-loop-edge")
        );

        let config = crate::flowchart::FlowchartConfigView::new(
            artifact.metadata.effective_config.as_value(),
        );
        let font_family = config.font_family();
        let render_style = config.render_text_style(&font_family, config.render_font_size());
        let edge_width = config.layout_settings().edge_label_wrapping_width;
        let render_measurer = artifact
            .context
            .session
            .text_measurer(crate::environment::TextMeasurementPhase::SvgBBox);
        let calls_before = text_measurement_call_count(&artifact.context.session);
        let plan = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
            Some(flowchart.svg_label_sidecar()),
            Some(owner),
            label,
            &render_measurer,
            &render_style,
            Some(edge_width),
            true,
            crate::flowchart::FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(
            &plan,
            crate::flowchart::FlowchartSvgLabelRenderPlan::Prepared { .. }
        ));
        let wrapped = plan.wrapped_lines();
        assert!(matches!(&wrapped, std::borrow::Cow::Borrowed(_)));
        assert!(wrapped.len() >= 2, "{wrapped:?}");
        assert_eq!(
            text_measurement_call_count(&artifact.context.session),
            calls_before,
            "a prepared self-loop label must not invoke the SVG measurer again"
        );
        drop(wrapped);
        drop(plan);

        let hits_before_render = flowchart.svg_label_sidecar().prepared_hit_count(owner);
        let svg = render_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
        )
        .expect("render self-loop SVG");
        assert!(
            flowchart.svg_label_sidecar().prepared_hit_count(owner) > hits_before_render,
            "the real Flowchart SVG renderer must consume the prepared self-loop label"
        );
        svg
    };

    assert!(!rendered_svg.contains("cyclic-special"), "{}", rendered_svg);
    let document = roxmltree::Document::parse(&rendered_svg).expect("valid self-loop SVG");
    let logical_path = document.descendants().any(|node| {
        node.has_tag_name("path") && node.attribute("data-id") == Some("self-loop-edge")
    });
    assert!(logical_path, "{rendered_svg}");
    let label_group = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some("self-loop-edge"))
        .expect("logical self-loop label group");
    let text = label_group
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .collect::<String>();
    assert!(text.contains("self loop semantic owner"), "{text:?}");
    let row_count = label_group
        .descendants()
        .filter(|node| {
            node.has_tag_name("tspan")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "text-outer-tspan")
                })
        })
        .count();
    assert!(row_count >= 2, "rows={row_count}: {rendered_svg}");
}

#[test]
fn prepared_swimlane_edge_label_is_consumed_by_the_real_svg_renderer() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
swimlane-beta LR
A styled@-->|swimlane semantic owner keeps wrapped label rows through the generated labelRect| B
linkStyle default font-size:24px,font-weight:bold
linkStyle 0 font-size:12px,font-style:italic
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Swimlane")
        .expect("detect Swimlane");
    let artifact = prepare(parsed, &LayoutOptions::default(), session())
        .expect("prepare Swimlane family artifact");

    let rendered_svg = {
        let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
            panic!("expected Swimlane family artifact");
        };
        let model = crate::flowchart::FlowchartRenderModelRef::new(
            swimlane.pair().semantic(),
            swimlane.label_sources(),
        );
        let edge = model.edges.first().expect("styled Swimlane edge");
        assert_eq!(edge.id, "styled");
        let label = model
            .edge_label_for_render(0, edge)
            .expect("Swimlane edge label");
        let owner = swimlane
            .svg_label_sidecar()
            .edge_owner(edge.id.as_str(), true)
            .expect("semantic Swimlane edge owner");
        assert_eq!(
            owner,
            crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(0)
        );
        assert_eq!(
            swimlane.pair().layout().edges[0].label_node_id.as_deref(),
            Some("edge-label-A-B-styled")
        );

        let config = crate::flowchart::FlowchartConfigView::new(
            artifact.metadata.effective_config.as_value(),
        );
        let font_family = config.font_family();
        let base_style = config.render_text_style(&font_family, config.render_font_size());
        let default_edge_styles = model
            .edge_defaults
            .as_ref()
            .map_or(&[][..], |defaults| defaults.style.as_slice());
        let label_style = crate::flowchart::flowchart_swimlane_label_rect_text_style(
            &base_style,
            default_edge_styles,
            &edge.style,
        );
        let render_measurer = artifact
            .context
            .session
            .text_measurer(crate::environment::TextMeasurementPhase::SvgBBox);
        let plan = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
            Some(swimlane.svg_label_sidecar()),
            Some(owner),
            label,
            &render_measurer,
            label_style.as_ref(),
            Some(config.render_wrapping_width()),
            true,
            crate::flowchart::FlowchartSvgWidthMode::Bbox,
        );
        assert!(matches!(
            &plan,
            crate::flowchart::FlowchartSvgLabelRenderPlan::Prepared { .. }
        ));
        let wrapped = plan.wrapped_lines();
        assert!(matches!(&wrapped, std::borrow::Cow::Borrowed(_)));
        assert!(wrapped.len() >= 2, "{wrapped:?}");
        drop(wrapped);
        drop(plan);

        let hits_before_render = swimlane.svg_label_sidecar().prepared_hit_count(owner);
        let svg = render_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
        )
        .expect("render Swimlane SVG");
        assert!(
            swimlane.svg_label_sidecar().prepared_hit_count(owner) > hits_before_render,
            "the real Swimlane SVG renderer must consume the prepared labelRect"
        );
        svg
    };

    let document = roxmltree::Document::parse(&rendered_svg).expect("valid Swimlane SVG");
    let label_group = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-et") == Some("edge-label")
                && node.attribute("data-id") == Some("styled")
        })
        .expect("generated labelRect group");
    let visible = label_group
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .flat_map(str::chars)
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    assert_eq!(
        visible,
        "swimlanesemanticownerkeepswrappedlabelrowsthroughthegeneratedlabelRect"
    );
}

fn prepare_with_model_item_limit(
    source: &str,
    max_model_items: usize,
) -> Result<FamilyRenderArtifact> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxModelItems,
                    max_model_items,
                )
                .unwrap(),
        )
        .begin_session()
        .unwrap();
    prepare(parsed, &LayoutOptions::default(), session)
}

fn prepare_with_layout_work_limit(
    source: &str,
    max_layout_work_units: usize,
) -> Result<FamilyRenderArtifact> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxLayoutWorkUnits,
                    max_layout_work_units,
                )
                .unwrap(),
        )
        .begin_session()
        .unwrap();
    prepare(parsed, &LayoutOptions::default(), session)
}

fn prepare_with_prepared_text_retained_limit(
    source: &str,
    max_prepared_text_retained_bytes: Option<usize>,
) -> Result<FamilyRenderArtifact> {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let typography = ThemeTextStyle::default()
        .with_font_stack(crate::diagram_theme::FontStack::single("Excalifont").unwrap());
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_default(typography))
                .with_assets(
                    crate::diagram_theme::ThemeAssets::default().with_font_catalog(
                        crate::diagram_theme::FontCatalogSpec::new([
                            crate::diagram_theme::FontAssetSpec::new("excalifont", bytes),
                        ]),
                    ),
                ),
        )
        .expect("prepared-text budget fixture theme should compile");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("source should produce a render model");
    let mut policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input();
    if let Some(maximum) = max_prepared_text_retained_bytes {
        policy = policy
            .with_limit(
                crate::resources::ResourceLimitId::MaxPreparedTextRetainedBytes,
                maximum,
            )
            .unwrap();
    }
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session_with_theme(&theme)
        .unwrap();
    prepare(parsed, &LayoutOptions::default(), session)
}

fn prepare_with_unbounded_layout_work(source: &str) -> Result<FamilyRenderArtifact> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(crate::resources::RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    prepare(parsed, &LayoutOptions::default(), session)
}

fn assert_model_item_limit(error: Error, actual: usize, max: usize) {
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_model_items resource limit error")
    };
    assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(limit.limit, "max_model_items");
    assert_eq!(limit.actual, actual);
    assert_eq!(limit.max, max);
}

#[test]
fn session_report_accounts_for_every_metered_layout_family() {
    let cases = vec![
        (
            "classDiagram\nclass A\nclass B\nA --> B\n",
            DiagramFamilyId::CLASS,
        ),
        (
            "stateDiagram-v2\n[*] --> Idle\nIdle --> Active\n",
            DiagramFamilyId::STATE,
        ),
        (
            "erDiagram\nCUSTOMER ||--o{ ORDER : places\n",
            DiagramFamilyId::ER,
        ),
        (
            "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    First child\n    Second child\n",
            DiagramFamilyId::MINDMAP,
        ),
        (
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: hello\n",
            DiagramFamilyId::SEQUENCE,
        ),
        (
            "kanban\n  todo[Todo]\n    task[Task]\n",
            DiagramFamilyId::KANBAN,
        ),
        (
            "requirementDiagram\nrequirement req1 {\n  id: 1\n  text: Login\n  risk: high\n}\n",
            DiagramFamilyId::REQUIREMENT,
        ),
        ("sankey-beta\nA,B,10\n", DiagramFamilyId::SANKEY),
        (
            "radar-beta\naxis A,B,C\ncurve score{1,2,3}\n",
            DiagramFamilyId::RADAR,
        ),
        (
            "venn-beta\nset A[\"Core\"]:20\nset B[\"Editor\"]:14\nunion A,B[\"Shared\"]:4\n",
            DiagramFamilyId::VENN,
        ),
    ];

    #[cfg(feature = "layout-cytoscape")]
    let cases = {
        let mut cases = cases;
        cases.push((
            "mindmap\n  Root\n    First child\n    Second child\n",
            DiagramFamilyId::MINDMAP,
        ));
        cases
    };

    for (source, expected_family) in cases {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::default())
            .unwrap()
            .expect("the layout-work fixture should produce a render model");
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

        assert_eq!(artifact.family_id(), expected_family);
        assert!(
            artifact.context.session().report().layout_work_units() > 0,
            "{expected_family} must contribute layout work to the session report"
        );
    }
}

#[test]
fn prepared_text_retained_budget_has_an_exact_family_boundary() {
    let cases = [
        ("stateDiagram-v2\nReady --> Done\n", DiagramFamilyId::STATE),
        (
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA -->|portable label| B\n",
            DiagramFamilyId::FLOWCHART,
        ),
    ];

    for (source, expected_family) in cases {
        let unbounded = prepare_with_prepared_text_retained_limit(source, None).unwrap();
        assert_eq!(unbounded.family_id(), expected_family);
        let exact = unbounded
            .context
            .session
            .report()
            .prepared_text_retained_bytes_peak();
        assert!(exact > 1, "{expected_family} must retain prepared text");
        drop(unbounded);

        let bounded = prepare_with_prepared_text_retained_limit(source, Some(exact)).unwrap();
        assert_eq!(
            bounded
                .context
                .session
                .report()
                .prepared_text_retained_bytes_peak(),
            exact
        );
        drop(bounded);

        let error = match prepare_with_prepared_text_retained_limit(source, Some(exact - 1)) {
            Ok(_) => panic!("{expected_family} exact minus one unexpectedly succeeded"),
            Err(error) => error,
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected {expected_family} prepared-text retained-byte rejection")
        };
        assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
        assert_eq!(limit.max, exact - 1);
        assert_eq!(limit.actual, exact);
    }
}

#[test]
fn invalidated_resvg_output_releases_prepared_text_evidence_and_reservations() {
    let artifact = prepare_with_prepared_text_retained_limit(
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA -->|portable label| B\n",
            None,
        )
        .unwrap();
    let work_meter = Arc::clone(artifact.context.session.work_meter());
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(work_meter.prepared_text_retained_bytes() > 0);

    let finalized = rendered
        .finalize_resvg(
            &SvgPipeline::resvg_safe()
                .with_postprocessor(crate::svg::RootBackgroundPostprocessor::new("white")),
        )
        .unwrap();

    assert!(!finalized.svg().prepared_text_evidence_valid());
    assert!(finalized.svg().prepared_text_label_ledger().is_empty());
    assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn sealed_resvg_output_shares_the_prepared_text_reservation_lease() {
    let artifact = prepare_with_prepared_text_retained_limit(
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA -->|portable label| B\n",
            None,
        )
        .unwrap();
    let work_meter = Arc::clone(artifact.context.session.work_meter());
    let finalized = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .unwrap();
    assert!(finalized.svg().prepared_text_evidence_valid());
    assert!(!finalized.svg().prepared_text_label_ledger().is_empty());
    assert!(work_meter.prepared_text_retained_bytes() > 0);

    let (output, report) = finalized.into_completion().into_output_and_report();
    let output_clone = output.clone();
    assert!(work_meter.prepared_text_retained_bytes() > 0);
    drop(output);
    assert!(work_meter.prepared_text_retained_bytes() > 0);
    drop(output_clone);
    assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
    assert_eq!(report.family_id(), DiagramFamilyId::FLOWCHART);
}

#[test]
fn raw_svg_completion_discards_the_prepared_text_reservation_lease() {
    let artifact = prepare_with_prepared_text_retained_limit(
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA -->|portable label| B\n",
            None,
        )
        .unwrap();
    let work_meter = Arc::clone(artifact.context.session.work_meter());
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(work_meter.prepared_text_retained_bytes() > 0);

    let (svg, report) = rendered.into_completion().into_output_and_report();

    assert!(svg.starts_with("<svg"));
    assert_eq!(report.family_id(), DiagramFamilyId::FLOWCHART);
    assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
}

#[test]
fn dagre_family_work_budgets_are_exact_and_preserve_layout_output() {
    let cases = [
        (
            "classDiagram\nnamespace Outer {\n  class A\n  class B\n}\nA --> B\n",
            DiagramFamilyId::CLASS,
        ),
        (
            "stateDiagram-v2\nstate Parent {\n  [*] --> Idle\n  Idle --> Active\n}\nParent --> Outside\n",
            DiagramFamilyId::STATE,
        ),
        (
            "erDiagram\nNODE {\n  string id\n}\nNODE ||--o{ NODE : leads\n",
            DiagramFamilyId::ER,
        ),
    ];

    for (source, expected_family) in cases {
        let unbounded = prepare_with_unbounded_layout_work(source).unwrap();
        assert_eq!(unbounded.family_id(), expected_family);
        let exact = unbounded.context.session.report().layout_work_units();
        assert!(exact > 0, "{expected_family} must report layout work");
        let expected_layout = unbounded.layout_json().unwrap();

        let bounded = prepare_with_layout_work_limit(source, exact).unwrap();
        assert_eq!(bounded.context.session.report().layout_work_units(), exact);
        assert_eq!(bounded.layout_json().unwrap(), expected_layout);

        let error = match prepare_with_layout_work_limit(source, exact - 1) {
            Ok(_) => panic!("{expected_family} exact minus one unexpectedly succeeded"),
            Err(error) => error,
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected {expected_family} layout work rejection")
        };
        assert_eq!(limit.limit, "max_layout_work_units");
        assert_eq!(limit.max, exact - 1);
    }
}

#[cfg(feature = "layout-cytoscape")]
#[test]
fn default_mindmap_cose_reports_kernel_work_and_has_an_exact_resource_boundary() {
    let (unbounded_result, unbounded_host) = prepare_mindmap_with_host_limit(None);
    let unbounded = unbounded_result.expect("unbounded COSE mindmap");
    let exact = unbounded.context.session.report().layout_work_units();
    assert!(
        exact > 78,
        "kernel work must exceed the 3-node adapter estimate"
    );
    let unbounded_layout = unbounded.layout_json().expect("unbounded layout json");
    let unbounded_trace = unbounded_host.snapshot();
    assert!(!unbounded_trace.is_empty());

    let (exact_result, exact_host) = prepare_mindmap_with_host_limit(Some(exact));
    let exact_artifact = exact_result.expect("exact COSE budget");
    assert_eq!(
        exact_artifact.context.session.report().layout_work_units(),
        exact
    );
    assert_eq!(
        exact_artifact.layout_json().expect("exact layout json"),
        unbounded_layout
    );
    assert_eq!(exact_host.snapshot(), unbounded_trace);

    let (short_result, _short_host) = prepare_mindmap_with_host_limit(Some(exact - 1));
    let error = match short_result {
        Ok(_) => panic!("exact minus one must reject COSE work"),
        Err(error) => error,
    };
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_layout_work_units rejection")
    };
    assert_eq!(limit.limit, "max_layout_work_units");
    assert_eq!(limit.max, exact - 1);

    let (early_result, early_host) = prepare_mindmap_with_host_limit(Some(1));
    assert!(matches!(early_result, Err(Error::ResourceLimitExceeded(_))));
    assert!(
        early_host.snapshot().is_empty(),
        "adapter admission must reject before the first host measurement"
    );
}

#[test]
fn requirement_layout_projection_excludes_operation_prepared_labels() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            r#"requirementDiagram
requirement req1 {
  id: 1
  text: User logs in
  risk: high
}
element system {
  type: service
}
system - satisfies -> req1
"#,
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("Requirement source should produce a render model");
    let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout = &projection["layout"]["RequirementDiagram"];
    let fields = layout
        .as_object()
        .expect("Requirement layout projection should remain an object");

    assert_eq!(artifact.family_id(), DiagramFamilyId::REQUIREMENT);
    assert!(fields.contains_key("nodes"));
    assert!(fields.contains_key("edges"));
    assert!(fields.contains_key("bounds"));
    assert!(!fields.contains_key("labels"));
    assert!(!layout.to_string().contains("display_text"));
    let serialized_projection = projection.to_string();
    assert!(!serialized_projection.contains("max_width_px"));
    assert!(!serialized_projection.contains("keep_centered"));
    assert!(!serialized_projection.contains("divider_y_offset"));
    assert!(
        serde_json::from_value::<RequirementDiagramLayout>(layout.clone()).is_ok(),
        "prepared labels must not alter the public Requirement layout schema"
    );
}

#[test]
fn flowchart_family_renderer_reuses_prepared_labels_by_semantic_owner() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
subgraph S[Service title]
  A[Node label]
end
subgraph E[Empty title]
end
A labeled@-->|edge semantic owner wraps alpha beta gamma delta epsilon| B[Second node]
"#,
            ParseOptions::strict(),
        )
        .expect("parse Flowchart")
        .expect("detect Flowchart");
    let artifact = prepare(parsed, &LayoutOptions::default(), session())
        .expect("prepare Flowchart family artifact");

    let owners = {
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            panic!("expected Flowchart family artifact");
        };
        let model = crate::flowchart::FlowchartRenderModelRef::new(
            flowchart.pair().semantic(),
            flowchart.label_sources(),
        );
        let node_index = model
            .nodes
            .iter()
            .position(|node| node.id == "A")
            .expect("semantic node A");
        let empty_subgraph_index = model
            .subgraphs
            .iter()
            .position(|subgraph| subgraph.id == "E")
            .expect("semantic empty subgraph E");
        let cluster_index = model
            .subgraphs
            .iter()
            .position(|subgraph| subgraph.id == "S")
            .expect("semantic cluster S");
        let edge_index = model
            .edges
            .iter()
            .position(|edge| edge.id == "labeled")
            .expect("semantic labeled edge");
        let sidecar = flowchart.svg_label_sidecar();

        let owners = [
            sidecar.node_owner("A", false).expect("node owner"),
            sidecar
                .node_owner("E", false)
                .expect("empty subgraph owner"),
            sidecar.edge_owner("labeled", false).expect("edge owner"),
            sidecar
                .subgraph_title_owner("S")
                .expect("cluster title owner"),
        ];
        assert_eq!(
            owners,
            [
                crate::flowchart::FlowchartSvgLabelOwner::Node(node_index),
                crate::flowchart::FlowchartSvgLabelOwner::EmptySubgraphNode(empty_subgraph_index,),
                crate::flowchart::FlowchartSvgLabelOwner::Edge(edge_index),
                crate::flowchart::FlowchartSvgLabelOwner::SubgraphTitle(cluster_index),
            ]
        );
        owners
    };

    let (prepared_hits_before, source_plans_before) = {
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            unreachable!();
        };
        let sidecar = flowchart.svg_label_sidecar();
        (
            owners.map(|owner| sidecar.prepared_hit_count(owner)),
            owners.map(|owner| sidecar.source_plan_count(owner)),
        )
    };

    let svg = render_family_artifact_svg(
        &artifact,
        &SvgRenderOptions::default(),
        &SvgDebugOptions::default(),
    )
    .expect("render Flowchart SVG");

    let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
        unreachable!();
    };
    let sidecar = flowchart.svg_label_sidecar();
    for (index, owner) in owners.into_iter().enumerate() {
        let prepared_hits = sidecar.prepared_hit_count(owner);
        let source_plans = sidecar.source_plan_count(owner);
        assert!(
            prepared_hits > prepared_hits_before[index],
            "real Flowchart SVG emission must consume {owner:?}; prepared_hits={prepared_hits}, source_plans={source_plans}"
        );
        assert_eq!(
            source_plans, source_plans_before[index],
            "eligible owner {owner:?} must not fall back to render-time preparation"
        );
    }
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let visible_text = document
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .flat_map(str::chars)
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(visible_text.contains("Servicetitle"), "{visible_text}");
    assert!(visible_text.contains("Emptytitle"), "{visible_text}");
    assert!(visible_text.contains("edgesemanticowner"), "{visible_text}");
}

#[test]
fn custom_catalog_swimlane_group_title_fails_closed_until_markdown_is_prepared() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let typography = ThemeTextStyle::default()
        .with_font_stack(crate::diagram_theme::FontStack::single("Excalifont").unwrap());
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_default(typography))
                .with_assets(
                    crate::diagram_theme::ThemeAssets::default().with_font_catalog(
                        crate::diagram_theme::FontCatalogSpec::new([
                            crate::diagram_theme::FontAssetSpec::new("excalifont", bytes),
                        ]),
                    ),
                ),
        )
        .expect("fixture theme should compile");
    let parsed = theme
        .install_parse_compatibility(Engine::new())
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
subgraph Lane[Portable lane]
  A[Node]
end
"#,
            ParseOptions::strict(),
        )
        .expect("parse Swimlane")
        .expect("detect Swimlane");
    let session = crate::environment::RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin themed session");

    assert!(matches!(
        prepare(parsed, &LayoutOptions::default(), session),
        Err(Error::TextLayout(
            crate::text::TextLayoutFailure::UnsupportedLabelMode
        ))
    ));
}

#[test]
fn flowchart_special_shape_intersections_reuse_layout_label_metrics() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
P[plain]
S([stadium label])
H{{hexagon label}}
D@{ shape: doc, label: "document label", labelType: "string" }
P --> S
P --> H
P --> D
S --> P
H --> P
D --> P
"#,
            ParseOptions::strict(),
        )
        .expect("parse Flowchart")
        .expect("detect Flowchart");
    let artifact = prepare(parsed, &LayoutOptions::default(), session())
        .expect("prepare Flowchart family artifact");
    let operation_count =
        |artifact: &FamilyRenderArtifact,
         operation: crate::environment::TextMeasurementOperation| {
            artifact
                .context
                .session()
                .text_measurement_report()
                .entries()
                .iter()
                .filter(|entry| entry.provenance().operation == operation)
                .map(|entry| entry.count())
                .sum::<u64>()
        };
    let operation_counts = |artifact: &FamilyRenderArtifact| {
        [
            operation_count(
                artifact,
                crate::environment::TextMeasurementOperation::Wrapped,
            ),
            operation_count(
                artifact,
                crate::environment::TextMeasurementOperation::ComputedLength,
            ),
        ]
    };
    let operations_before = operation_counts(&artifact);
    let (owners, source_plans_before) = {
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            panic!("expected Flowchart family artifact");
        };
        let node_ids = flowchart
            .pair()
            .semantic()
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        let sidecar = flowchart.svg_label_sidecar();
        let owners = ["S", "H", "D"].map(|id| {
            sidecar.node_owner(id, false).unwrap_or_else(|| {
                panic!("missing semantic label owner for {id}; nodes={node_ids:?}")
            })
        });
        (owners, owners.map(|owner| sidecar.source_plan_count(owner)))
    };

    let svg = render_family_artifact_svg(
        &artifact,
        &SvgRenderOptions::default(),
        &SvgDebugOptions::default(),
    )
    .expect("render Flowchart SVG");

    assert_eq!(
        operation_counts(&artifact),
        operations_before,
        "special-shape edge intersections must consume layout label metrics"
    );
    let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
        unreachable!();
    };
    for (index, owner) in owners.into_iter().enumerate() {
        assert_eq!(
            flowchart.svg_label_sidecar().source_plan_count(owner),
            source_plans_before[index],
            "special-shape label {owner:?} must reuse its prepared SVG plan"
        );
    }
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let visible_text = document
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .flat_map(str::chars)
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(visible_text.contains("stadiumlabel"), "{visible_text}");
    assert!(visible_text.contains("hexagonlabel"), "{visible_text}");
    assert!(visible_text.contains("documentlabel"), "{visible_text}");
}

#[test]
fn swimlane_family_renderer_reuses_the_original_semantic_edge_owner() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
swimlane-beta LR
A --> C
A styled@-->|swimlane semantic owner wraps alpha beta gamma delta epsilon| B
"#,
            ParseOptions::strict(),
        )
        .expect("parse Swimlane")
        .expect("detect Swimlane");
    let artifact = prepare(parsed, &LayoutOptions::default(), session())
        .expect("prepare Swimlane family artifact");

    let (owner, hits_before, source_plans_before) = {
        let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
            panic!("expected Swimlane family artifact");
        };
        let model = crate::flowchart::FlowchartRenderModelRef::new(
            swimlane.pair().semantic(),
            swimlane.label_sources(),
        );
        let edge_index = model
            .edges
            .iter()
            .position(|edge| edge.id == "styled")
            .expect("semantic styled edge");
        let owner = swimlane
            .svg_label_sidecar()
            .edge_owner("styled", true)
            .expect("Swimlane edge-label owner");
        assert_eq!(
            owner,
            crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(edge_index)
        );
        assert_eq!(
            swimlane
                .pair()
                .layout()
                .edges
                .iter()
                .find(|edge| edge.id == "styled")
                .and_then(|edge| edge.label_node_id.as_deref()),
            Some("edge-label-A-B-styled")
        );
        (
            owner,
            swimlane.svg_label_sidecar().prepared_hit_count(owner),
            swimlane.svg_label_sidecar().source_plan_count(owner),
        )
    };

    let svg = render_family_artifact_svg(
        &artifact,
        &SvgRenderOptions::default(),
        &SvgDebugOptions::default(),
    )
    .expect("render Swimlane SVG");

    let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
        unreachable!();
    };
    assert!(
        swimlane.svg_label_sidecar().prepared_hit_count(owner) > hits_before,
        "real Swimlane SVG emission must consume the prepared labelRect"
    );
    assert_eq!(
        swimlane.svg_label_sidecar().source_plan_count(owner),
        source_plans_before
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Swimlane SVG");
    let visible_text = document
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .flat_map(str::chars)
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(
        visible_text.contains("swimlanesemanticowner"),
        "{visible_text}"
    );
}

#[test]
fn flowchart_family_layout_projections_exclude_operation_prepared_labels() {
    for (source, variant) in [
        ("flowchart LR\nA -->|Flowchart label| B\n", "FlowchartV2"),
        (
            "swimlane-beta LR\nA -->|Swimlane label| B\n",
            "SwimlaneDiagram",
        ),
    ] {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse Flowchart family")
            .expect("detect Flowchart family");
        let artifact = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare Flowchart family artifact");
        let projection = artifact.layout_json().expect("project public layout JSON");
        let layout = &projection["layout"][variant];
        let serialized = layout.to_string();

        for internal_field in [
            "wrapped_lines",
            "plain_text",
            "binding",
            "render_ids",
            "svg_label_sidecar",
        ] {
            assert!(
                !serialized.contains(internal_field),
                "{variant} leaked operation-local field {internal_field}: {serialized}"
            );
        }

        match variant {
            "FlowchartV2" => assert!(
                serde_json::from_value::<FlowchartLayout>(layout.clone()).is_ok(),
                "prepared labels must not alter the public Flowchart layout schema"
            ),
            "SwimlaneDiagram" => assert!(
                serde_json::from_value::<SwimlaneLayout>(layout.clone()).is_ok(),
                "prepared labels must not alter the public Swimlane layout schema"
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn dagre_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
    let source = "flowchart TD\nA --> B";
    let artifact = prepare_with_model_item_limit(source, 3).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::FLOWCHART);

    let error = match prepare_with_model_item_limit(source, 2) {
        Err(error) => error,
        Ok(_) => panic!("flowchart above the node limit unexpectedly rendered"),
    };
    assert_model_item_limit(error, 3, 2);
}

#[test]
fn swimlane_node_limit_accepts_boundary_and_rejects_one_beyond() {
    let source = "swimlane-beta LR\nA --> B";
    let artifact = prepare_with_model_item_limit(source, 3).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::SWIMLANE);

    let error = match prepare_with_model_item_limit(source, 2) {
        Err(error) => error,
        Ok(_) => panic!("swimlane above the node limit unexpectedly rendered"),
    };
    assert_model_item_limit(error, 3, 2);
}

#[test]
fn swimlane_rejects_pairwise_routing_work_before_layout() {
    let source = "swimlane-beta LR\nA --> B\nB --> C";
    let artifact = prepare_with_layout_work_limit(source, 1_000).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::SWIMLANE);

    let error = match prepare_with_layout_work_limit(source, 1) {
        Err(error) => error,
        Ok(_) => panic!("swimlane above the layout work limit unexpectedly rendered"),
    };
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_layout_work_units resource limit error");
    };
    assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(limit.limit, "max_layout_work_units");
    assert!(limit.actual > limit.max);
    assert_eq!(limit.max, 1);
}

#[test]
fn mindmap_node_limit_is_checked_before_layout_allocation_or_backend_dispatch() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            "mindmap\n  Root\n    First child\n    Second child\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("mindmap source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxModelItems, 4)
                .unwrap(),
        )
        .begin_session()
        .unwrap();

    let error = match prepare(parsed, &LayoutOptions::default(), session) {
        Err(error) => error,
        Ok(_) => panic!("mindmap above the node limit unexpectedly reached layout"),
    };
    let Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected max_model_items resource limit error");
    };
    assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
    assert_eq!(limit.limit, "max_model_items");
    assert_eq!(limit.actual, 5);
    assert_eq!(limit.max, 4);
}

#[test]
fn flowchart_math_capability_uses_parser_owned_render_spelling() {
    for source in [
        "flowchart TD\nA[\"#36;#36;node#36;#36;\"]\n",
        "flowchart TD\nA -->|#36;#36;edge#36;#36;| B\n",
        "flowchart TD\nsubgraph S[\"#36;#36;group#36;#36;\"]\nA\nend\n",
    ] {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert!(
            !plan
                .required_capabilities()
                .contains(&RenderCapability::Math),
            "encoded dollar entities remain ordinary createText input: {source}"
        );
        prepare(parsed, &LayoutOptions::default(), session)
            .expect("encoded dollar entities must not require a Math renderer");
    }

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA[\"$$x$$\"]\n", ParseOptions::strict())
        .unwrap()
        .expect("Flowchart source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .without_math_renderer()
        .begin_session()
        .unwrap();
    let plan = plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
    assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
}

#[test]
fn mindmap_math_label_requires_the_math_capability() {
    let source = r#"---
config:
  layout: tidy-tree
---
mindmap
  root[Root]
    formula["$$x^2$$"]
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("mindmap source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .without_math_renderer()
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
    assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
    assert!(!plan.is_ready());

    let error = match prepare(parsed, &LayoutOptions::default(), session) {
        Err(error) => error,
        Ok(_) => panic!("mindmap math label unexpectedly rendered without a math backend"),
    };
    assert!(matches!(
        error,
        Error::MissingCapability {
            capability: RenderCapability::Math,
            ref diagram_type,
        } if diagram_type == "mindmap"
    ));
}

#[derive(Debug)]
struct MindmapMathRenderer;

impl crate::math::MathRenderer for MindmapMathRenderer {
    fn render_html_label(
        &self,
        text: &str,
        _config: &merman_core::MermaidConfig,
    ) -> Option<String> {
        text.contains("$$")
            .then(|| "<strong>rendered-mindmap-math</strong>".to_string())
    }

    fn measure_html_label(
        &self,
        text: &str,
        _config: &merman_core::MermaidConfig,
        _style: &crate::text::TextStyle,
        _max_width_px: Option<f64>,
        _wrap_mode: crate::text::WrapMode,
    ) -> Option<crate::text::TextMetrics> {
        text.contains("$$").then_some(crate::text::TextMetrics {
            width: 96.0,
            height: 24.0,
            line_count: 1,
        })
    }
}

#[test]
fn mindmap_math_label_is_consumed_by_the_math_renderer() {
    let source = r#"---
config:
  layout: tidy-tree
---
mindmap
  root[Root]
    formula["$$x^2$$"]
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("mindmap source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(MindmapMathRenderer))
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
    assert!(plan.missing_capabilities().is_empty());
    assert!(plan.is_ready());

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(rendered.svg().contains("rendered-mindmap-math"));
    assert!(!rendered.svg().contains("$$x^2$$"));
}

#[test]
fn class_math_label_requires_the_math_capability() {
    let source = r#"classDiagram
class Formula["$$x^2$$"]
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Class source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .without_math_renderer()
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
    assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
    assert!(!plan.is_ready());

    let error = match prepare(parsed, &LayoutOptions::default(), session) {
        Err(error) => error,
        Ok(_) => panic!("Class math label unexpectedly rendered without a math backend"),
    };
    assert!(matches!(
        error,
        Error::MissingCapability {
            capability: RenderCapability::Math,
            ref diagram_type,
        } if diagram_type == "class"
    ));
}

#[derive(Debug)]
struct ClassMathRenderer;

impl crate::math::MathRenderer for ClassMathRenderer {
    fn render_html_label(
        &self,
        text: &str,
        _config: &merman_core::MermaidConfig,
    ) -> Option<String> {
        text.contains("$$")
            .then(|| "<div>rendered-class-math</div>".to_string())
    }

    fn measure_html_label(
        &self,
        text: &str,
        _config: &merman_core::MermaidConfig,
        _style: &crate::text::TextStyle,
        _max_width_px: Option<f64>,
        _wrap_mode: crate::text::WrapMode,
    ) -> Option<crate::text::TextMetrics> {
        text.contains("$$").then_some(crate::text::TextMetrics {
            width: 96.0,
            height: 24.0,
            line_count: 1,
        })
    }
}

#[test]
fn class_math_label_is_consumed_by_the_math_renderer() {
    let source = r#"classDiagram
class Formula["$$x^2$$"]
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Class source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(ClassMathRenderer))
        .begin_session()
        .unwrap();

    let plan = plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
    assert!(plan.missing_capabilities().is_empty());
    assert!(plan.is_ready());

    let rendered = prepare(parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(rendered.svg().contains("rendered-class-math"));
    assert!(!rendered.svg().contains("$$x^2$$"));
}

fn render_class_math(source: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .expect("Class source should produce a render model");
    let session = crate::environment::RenderEnvironment::deterministic()
        .with_math_renderer(std::sync::Arc::new(ClassMathRenderer))
        .begin_session()
        .unwrap();
    prepare(parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_string()
}

#[test]
fn class_math_label_forces_html_rendering_when_html_labels_are_disabled() {
    let svg = render_class_math(
        r#"---
config:
  htmlLabels: false
---
classDiagram
class Formula["$$x^2$$"]
"#,
    );

    assert!(svg.contains("rendered-class-math"));
    assert!(!svg.contains("$$x^2$$"));
}

#[test]
fn class_relation_terminal_and_note_math_labels_use_the_math_renderer() {
    let svg = render_class_math(
        r#"classDiagram
class Formula
class Result
Formula "$$one$$" --> "$$many$$" Result : $$edge$$
note for Formula "$$note$$"
"#,
    );

    assert_eq!(svg.matches("rendered-class-math").count(), 4);
    assert!(!svg.contains("$$"));
    assert!(!svg.contains("<p><div>"));
}

#[test]
fn class_annotation_and_interface_math_labels_require_and_use_math() {
    for source in [
        r#"classDiagram
class Formula <<$$annotation$$>>
"#,
        r#"classDiagram
class Formula
$$interface$$ ()-- Formula
"#,
    ] {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);

        let svg = render_class_math(source);
        assert!(svg.contains("rendered-class-math"));
        assert!(!svg.contains("$$"));
        assert!(!svg.contains("<p><div>"));
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn elk_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
    let source = "flowchart-elk TD\nA --> B";
    let artifact = prepare_with_model_item_limit(source, 3).unwrap();
    assert_eq!(artifact.family_id(), DiagramFamilyId::FLOWCHART);

    let error = match prepare_with_model_item_limit(source, 2) {
        Err(error) => error,
        Ok(_) => panic!("ELK flowchart above the node limit unexpectedly rendered"),
    };
    assert_model_item_limit(error, 3, 2);
}

#[test]
fn custom_semantic_json_is_explicitly_non_renderable() {
    let mut engine = Engine::new();
    engine
        .diagram_registry_mut()
        .insert("customDiagram", custom_semantic_parser);
    let parsed = engine
        .parse_diagram_for_render_model_with_type_sync(
            "customDiagram",
            "customDiagram\npayload",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();

    let error = match prepare(parsed, &LayoutOptions::default(), session()) {
        Err(error) => error,
        Ok(_) => panic!("custom JSON unexpectedly produced a built-in artifact"),
    };
    let Error::NonRenderableCustomModel {
        diagram_type,
        model_name,
        provenance,
    } = error
    else {
        panic!("expected explicit custom-model capability error")
    };
    assert_eq!(diagram_type, "customDiagram");
    assert_eq!(model_name, "customDiagram");
    assert_eq!(provenance, CustomJsonProvenance::SemanticRegistryOverlay);
}

#[test]
fn custom_render_overlay_cannot_masquerade_as_a_builtin_family() {
    let mut engine = Engine::new();
    engine
        .render_diagram_registry_mut()
        .insert("flowchart-v2", custom_render_parser);
    let parsed = engine
        .parse_diagram_for_render_model_with_type_sync(
            "flowchart-v2",
            "flowchart TD\nA --> B",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();

    let error = match prepare(parsed, &LayoutOptions::default(), session()) {
        Err(error) => error,
        Ok(_) => panic!("custom JSON unexpectedly produced a built-in artifact"),
    };
    let Error::NonRenderableCustomModel {
        diagram_type,
        model_name,
        provenance,
    } = error
    else {
        panic!("expected explicit custom-model capability error")
    };
    assert_eq!(diagram_type, "flowchart-v2");
    assert_eq!(model_name, "custom-flowchart");
    assert_eq!(provenance, CustomJsonProvenance::RenderRegistryOverlay);
}

#[test]
fn gantt_time_axis_diagnostics_invert_rendered_x_without_exposing_layout() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  gantt:
    useWidth: 130
    leftPadding: 10
    rightPadding: 20
---
gantt
dateFormat x
section Delivery
First: first,-1,1ms
Second: second,after first,2ms
"#,
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

    assert_eq!(artifact.family_id(), DiagramFamilyId::GANTT);
    let diagnostics = artifact
        .gantt_time_axis_diagnostics()
        .expect("Gantt tasks should expose time-axis diagnostics");
    assert_eq!(diagnostics.unix_millis_at_rendered_x(10.0), Some(-1));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(43.0), Some(0));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(110.0), Some(2));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(44.0), None);
    assert_eq!(diagnostics.unix_millis_at_rendered_x(f64::NAN), None);

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
}

#[test]
fn suppressed_parse_failure_uses_the_typed_error_artifact_and_renderer() {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync("flowchart TD\nA -->", ParseOptions::lenient())
        .unwrap()
        .unwrap();
    let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

    assert_eq!(artifact.family_id(), DiagramFamilyId::ERROR);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    assert!(rendered.svg().contains("Syntax error in text"));
}
