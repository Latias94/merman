#![cfg(feature = "layout-cytoscape")]

type FamilyFixture = (
    merman_model::presentation::PresentationDiagram,
    merman_render::LayoutOptions,
    merman_render::environment::RenderSession,
);

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::ArchitectureDiagramLayout;
use merman_render::svg::{IconPack, IconRegistry, SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMetrics, TextStyle};
use merman_render::{DiagramFamilyId, LayoutOptions};
use regex::Regex;
use serde_json::json;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// Full architecture SVG rendering includes the public family pipeline; keep its platform
// headroom separate from the focused lower-level small-stack traversal tests.
#[cfg(windows)]
const DEEP_ARCHITECTURE_RENDER_STACK_SIZE: usize = 512 * 1024;
#[cfg(not(windows))]
const DEEP_ARCHITECTURE_RENDER_STACK_SIZE: usize = 128 * 1024;

#[derive(Default)]
struct CountingArchitectureHost {
    calls: AtomicUsize,
    operations: Mutex<Vec<TextMeasurementOperation>>,
    styles: Mutex<Vec<TextStyle>>,
    reject_generic_svg_measurement: std::sync::atomic::AtomicBool,
}

impl HostTextMeasurer for CountingArchitectureHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.operations
            .lock()
            .expect("Architecture host operations lock")
            .push(request.operation);
        self.styles
            .lock()
            .expect("Architecture host styles lock")
            .push(request.style.clone());
        let width = request.text.chars().count() as f64 * request.style.font_size.max(1.0);
        Ok(Some(match request.operation {
            TextMeasurementOperation::Measure | TextMeasurementOperation::Wrapped => {
                assert!(
                    !self.reject_generic_svg_measurement.load(Ordering::Relaxed),
                    "Architecture SVG must select a source-backed text primitive instead of generic measurement"
                );
                HostTextMeasurement::Metrics(TextMetrics {
                    width,
                    height: request.style.font_size.max(1.0),
                    line_count: 1,
                })
            }
            TextMeasurementOperation::ComputedLength => HostTextMeasurement::Length(width * 0.75),
            TextMeasurementOperation::TspanBBoxWidth => HostTextMeasurement::Length(width + 11.0),
            TextMeasurementOperation::BBoxX => HostTextMeasurement::HorizontalExtents {
                left: width / 2.0,
                right: width / 2.0,
            },
            TextMeasurementOperation::TspanBBoxHeight => HostTextMeasurement::Length(23.0),
            TextMeasurementOperation::CreateTextMiddleBBoxYOffset => {
                HostTextMeasurement::Length(7.0)
            }
            _ => return Ok(None),
        }))
    }
}

fn counting_architecture_environment(host: Arc<CountingArchitectureHost>) -> RenderEnvironment {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.architecture-host").expect("valid profile id"),
        "1",
    )
    .expect("valid profile identity");
    RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(identity, host, TextMeasurementPhase::ALL),
    )
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn render_architecture_fixture_with_options(
    fixture_name: &str,
    options: &SvgRenderOptions,
) -> String {
    let path = workspace_root()
        .join("fixtures")
        .join("architecture")
        .join(fixture_name);
    let text = std::fs::read_to_string(&path).expect("read fixture");

    render_architecture_text_with_options(&text, options)
}

fn render_architecture_text_with_options(text: &str, options: &SvgRenderOptions) -> String {
    let engine = MermaidEngine::new();
    render_architecture_text_with_engine_and_options(&engine, text, options)
}

fn render_architecture_text_with_engine_and_options(
    engine: &MermaidEngine,
    text: &str,
    options: &SvgRenderOptions,
) -> String {
    let (fixture_parsed_1, fixture_layout_1, fixture_session_1) =
        prepare_architecture_text_with_engine(engine, text);
    family::prepare(&fixture_parsed_1, &fixture_layout_1, fixture_session_1)
        .expect("prepare family fixture")
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned()
}

fn prepare_architecture_text_with_engine(engine: &MermaidEngine, text: &str) -> FamilyFixture {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let layout_options = LayoutOptions::headless_svg_defaults();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    (parsed, layout_options.clone(), session)
}

fn architecture_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::ARCHITECTURE, typography),
        ))
        .expect("compile Architecture typography theme")
}

#[test]
fn architecture_cytoscape_title_requests_are_not_applicable_without_a_diagram_title_dom() {
    for source in [
        "architecture-beta\ntitle Nonvisual diagram title\nservice api(server)[API]\n",
        "---\ntitle: Nonvisual diagram title\n---\narchitecture-beta\nservice api(server)[API]\n",
    ] {
        let render = |theme: &DiagramTheme| {
            let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
                source,
                theme,
                MermaidEngine::new(),
                RenderEnvironment::deterministic(),
            );
            family::prepare(&parsed, &options, session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap()
        };
        let baseline = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .unwrap();
        let baseline = render(&baseline);
        for variant in [
            None,
            Some(ThemeVariant::Default),
            Some(ThemeVariant::Primary),
        ] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Title,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#2468ac").unwrap())
                    .with_stroke(CanvasPaint::solid("#987654").unwrap()),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)),
                )
                .unwrap();
            let rendered = render(&theme);
            assert_eq!(rendered.svg(), baseline.svg());
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                document
                    .descendants()
                    .any(|node| node.is_text() && node.text() == Some("API"))
            );
            assert!(
                !document.descendants().any(|node| {
                    node.is_text() && node.text() == Some("Nonvisual diagram title")
                })
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn architecture_all_presets_keep_nonvisual_diagram_titles_not_applicable() {
    let presets = merman_render::diagram_theme::theme_preset_descriptors();
    assert_eq!(presets.len(), 11);
    let source = "---\ntitle: Nonvisual metadata title\n---\narchitecture-beta\ntitle Nonvisual diagram title\ngroup backend(cloud)[Backend]\nservice api(server)[API] in backend\n";
    for preset in presets {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset.preset())
            .unwrap();
        let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
            .begin_session_with_theme(&theme)
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for label in ["API", "Backend"] {
            assert!(
                document
                    .descendants()
                    .any(|node| node.is_text() && node.text() == Some(label))
            );
        }
        assert!(!document.descendants().any(|node| {
            node.is_text()
                && matches!(
                    node.text(),
                    Some("Nonvisual metadata title" | "Nonvisual diagram title")
                )
        }));
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert!(evidence.not_applicable_count() > 0, "{}", preset.id());
        assert_eq!(
            evidence.accounted_count(),
            evidence.required_count(),
            "{}",
            preset.id()
        );
        assert!(
            completion
                .report()
                .theme_diagnostics()
                .iter()
                .all(|diagnostic| {
                    diagnostic.target() != Some(ThemeTarget::Title)
                        && diagnostic.code() != "evidence-incomplete"
                }),
            "{}",
            preset.id()
        );
    }
}

fn architecture_text_fill_theme(fill: CanvasPaint, variant: Option<ThemeVariant>) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(fill),
    )
    .for_family(DiagramFamilyId::ARCHITECTURE);
    let rule = match variant {
        Some(variant) => rule.with_variant(variant),
        None => rule,
    };
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .expect("compile Architecture text theme")
}

fn prepare_architecture_with_theme_and_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    environment: RenderEnvironment,
) -> FamilyFixture {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Architecture")
        .expect("detect themed Architecture")
        .into_presentation();
    let session = environment
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Architecture session");
    (parsed, LayoutOptions::headless_svg_defaults(), session)
}

fn try_render_architecture_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(json!({
            "architecture": {"numIter": 1, "randomize": false}
        })),
    );
    let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
        source,
        theme,
        engine,
        RenderEnvironment::deterministic(),
    );
    family::prepare(&parsed, &options, session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

#[test]
fn architecture_group_clear_restores_baseline_with_complete_evidence() {
    use merman_render::diagram_theme::Specified;

    let source =
        "architecture-beta\ngroup backend(cloud)[Backend]\nservice api(server)[API] in backend";
    let baseline_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let baseline = try_render_architecture_with_theme(source, &baseline_theme).unwrap();
    let document = roxmltree::Document::parse(baseline.svg()).unwrap();
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect") && node.attribute("id") == Some("architecture-group-backend")
    }));

    for (clear_fill, clear_stroke) in [(true, false), (false, true), (true, true)] {
        for preceding_value in [false, true] {
            let mut value = ThemeStylePatch::default();
            let mut clear = ThemeStylePatch::default();
            if clear_fill {
                value.paint.fill = Specified::Value(CanvasPaint::solid("#123456").unwrap());
                clear.paint.fill = Specified::Clear;
            }
            if clear_stroke {
                value.stroke.paint = Specified::Value(CanvasPaint::solid("#654321").unwrap());
                clear.stroke.paint = Specified::Clear;
            }
            let mut rules = ThemeRuleSet::default();
            if preceding_value {
                rules = rules.with_rule(ThemeRule::new(ThemeTarget::Cluster, value));
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new()
                        .with_styles(rules.with_rule(ThemeRule::new(ThemeTarget::Cluster, clear))),
                )
                .unwrap();

            // Reproduction: Clear emits no inline style, but a real group still seals it.
            let rendered = try_render_architecture_with_theme(source, &theme)
                .expect("a checkpointed group must complete a winning Clear in strict mode");
            assert_eq!(rendered.svg(), baseline.svg());
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1 + usize::from(preceding_value));
            assert_eq!(evidence.accounted_count(), evidence.required_count());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(preceding_value)
            );
            assert_eq!(evidence.theme_residual_count(), 0);

            let absent = try_render_architecture_with_theme(
                "architecture-beta\nservice api(server)[API]",
                &theme,
            )
            .unwrap();
            let completion = absent.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.accounted_count(), evidence.required_count());
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), evidence.required_count());
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn architecture_edge_nonmatching_variants_are_not_applicable() {
    use merman_render::diagram_theme::OrdinalSelector;

    let source =
        "architecture-beta\nservice api(server)[API]\nservice db(database)[DB]\napi:R --> L:db";
    let baseline_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let baseline = try_render_architecture_with_theme(source, &baseline_theme).unwrap();
    let document = roxmltree::Document::parse(baseline.svg()).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("path") && node.attribute("class") == Some("edge") })
    );

    for ordinal in [None, Some(OrdinalSelector::Exact(1))] {
        let mut rule = ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#ff0000").unwrap()),
        )
        .with_variant(ThemeVariant::Primary);
        if let Some(ordinal) = ordinal {
            rule = rule.with_ordinal(ordinal);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();

        // Reproduction: Primary has no Architecture edge terminal, even at a real ordinal.
        let rendered = try_render_architecture_with_theme(source, &theme)
            .expect("a nonmatching variant must not create a strict-mode paint residual");
        assert_eq!(rendered.svg(), baseline.svg());
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn architecture_edge_overridden_default_variant_is_not_applicable() {
    let source =
        "architecture-beta\nservice api(server)[API]\nservice db(database)[DB]\napi:R --> L:db";
    let winner = ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap()),
    );
    let baseline_theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(winner.clone())),
        )
        .unwrap();
    let baseline = try_render_architecture_with_theme(source, &baseline_theme).unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#ff0000").unwrap()),
                        )
                        .with_variant(ThemeVariant::Default),
                    )
                    .with_rule(winner),
            ),
        )
        .unwrap();

    // Reproduction: matching Default must also win its property to produce a residual.
    let rendered = try_render_architecture_with_theme(source, &theme)
        .expect("a superseded Default request must not reject the supported winner");
    assert_eq!(rendered.svg(), baseline.svg());
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn architecture_edge_winning_default_variant_retains_unsupported_residual() {
    use merman_render::diagram_theme::OrdinalSelector;

    let source =
        "architecture-beta\nservice api(server)[API]\nservice db(database)[DB]\napi:R --> L:db";
    for ordinal in [None, Some(OrdinalSelector::Exact(1))] {
        let mut rule = ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#ff0000").unwrap()),
        )
        .with_variant(ThemeVariant::Default);
        if let Some(ordinal) = ordinal {
            rule = rule.with_ordinal(ordinal);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();

        // Guard: matching, winning Default selectors retain the existing unsupported contract.
        let error = try_render_architecture_with_theme(source, &theme)
            .err()
            .expect("an unsupported Default winner must remain rejected in strict mode");
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::ARCHITECTURE, 1))
        );
        assert_eq!(error.incomplete_family_theme(), None);
    }
}

fn render_architecture_fixture(fixture_name: &str) -> String {
    render_architecture_fixture_with_options(
        fixture_name,
        &SvgRenderOptions {
            diagram_id: Some("architecture-crosslinks".to_string()),
            ..Default::default()
        },
    )
}

#[test]
fn architecture_mixed_node_declarations_preserve_svg_order_and_accessibility() {
    let source = r#"architecture-beta
accTitle: Mixed nodes
accDescr: Services and junctions in nested groups
group outer(cloud)[Outer]
junction j2 in outer
service beta(server)[Beta] in outer
group inner(cloud)[Inner] in outer
junction j1 in inner
service alpha(database)[Alpha] in inner
alpha:R -[first]-> L:j1
beta:R -[second]-> L:j2
"#;
    let svg = render_architecture_text_with_options(
        source,
        &SvgRenderOptions {
            diagram_id: Some("architecture-mixed".to_owned()),
            ..Default::default()
        },
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Architecture SVG");
    let root = document.root_element();
    let group = |class| {
        root.children()
            .find(|node| node.attribute("class") == Some(class))
            .expect("Architecture DOM group")
    };
    let services = group("architecture-services");
    let node_ids = services
        .children()
        .filter(roxmltree::Node::is_element)
        .map(|node| {
            node.attribute("id")
                .or_else(|| node.descendants().find_map(|child| child.attribute("id")))
                .expect("service or junction identity")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        node_ids,
        [
            "architecture-mixed-service-beta",
            "architecture-mixed-service-alpha",
            "architecture-mixed-node-j2",
            "architecture-mixed-node-j1",
        ]
    );
    let edge_ids = group("architecture-edges")
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("class") == Some("edge"))
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        edge_ids,
        [
            "architecture-mixed-L_alpha_j1_0",
            "architecture-mixed-L_beta_j2_0"
        ]
    );
    let layers = root
        .children()
        .filter_map(|node| node.attribute("class"))
        .filter(|class| class.starts_with("architecture-"))
        .collect::<Vec<_>>();
    assert_eq!(
        layers,
        [
            "architecture-edges",
            "architecture-services",
            "architecture-groups"
        ]
    );
    let group_ids = group("architecture-groups")
        .children()
        .filter(|node| node.has_tag_name("rect"))
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        group_ids,
        [
            "architecture-mixed-group-outer",
            "architecture-mixed-group-inner"
        ]
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-architecture-mixed")
    );
    assert_eq!(
        root.attribute("aria-describedby"),
        Some("chart-desc-architecture-mixed")
    );
    assert!(
        root.children()
            .any(|node| node.has_tag_name("title") && node.text() == Some("Mixed nodes"))
    );
    assert!(root.children().any(|node| node.has_tag_name("desc")
        && node.text() == Some("Services and junctions in nested groups")));
}

#[test]
fn architecture_nested_group_surfaces_keep_connections_services_and_labels_in_front() {
    let source = r#"architecture-beta
group outer(cloud)[Outer]
group inner(database)[Inner] in outer
service api(server)[API] in inner
service worker[Worker] in outer
api:R -[request]-> L:worker
"#;
    for fill in ["#17324D", "#17324D80"] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Cluster,
                            ThemeStylePatch::default().with_fill(CanvasPaint::solid(fill).unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#c2188b").unwrap()),
                        )),
                ),
            )
            .unwrap();
        let rendered = try_render_architecture_with_theme(source, &theme).unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let root = document.root_element();
        let layers = root
            .children()
            .filter_map(|node| node.attribute("class"))
            .filter(|class| class.starts_with("architecture-"))
            .collect::<Vec<_>>();
        assert_eq!(
            layers,
            [
                "architecture-group-surfaces",
                "architecture-edges",
                "architecture-services",
                "architecture-groups",
            ],
            "{fill}"
        );
        let group = |class| {
            root.children()
                .find(|node| node.attribute("class") == Some(class))
                .unwrap()
        };
        let backgrounds = group("architecture-group-surfaces")
            .children()
            .filter(roxmltree::Node::is_element)
            .collect::<Vec<_>>();
        assert_eq!(backgrounds.len(), 2);
        for (background, id) in backgrounds.iter().zip(["outer", "inner"]) {
            assert!(background.has_tag_name("rect"));
            assert_eq!(
                background.attribute("id"),
                Some(format!("architecture-group-{id}").as_str())
            );
            assert_eq!(
                background.attribute("style"),
                Some(format!("fill:{fill};").as_str())
            );
        }
        let foreground = group("architecture-groups");
        assert!(!foreground.children().any(|node| node.has_tag_name("rect")));
        let labels = foreground
            .descendants()
            .filter(roxmltree::Node::is_text)
            .filter_map(|node| node.text())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["Outer", "Inner"]);
        assert!(
            foreground
                .descendants()
                .filter(|node| node.has_tag_name("svg"))
                .count()
                >= 2,
            "group icons must stay in the foreground: {fill}"
        );
        for label in foreground
            .descendants()
            .filter(|node| node.has_tag_name("text"))
        {
            assert_eq!(label.attribute("style"), Some("fill:#123456;"));
        }
        let edges = group("architecture-edges");
        assert!(edges.descendants().any(|node| {
            node.has_tag_name("path")
                && node.attribute("class") == Some("edge")
                && node.attribute("style") == Some("stroke:#c2188b;")
        }));
        assert!(
            edges
                .descendants()
                .any(|node| node.is_text() && node.text() == Some("request"))
        );
        let services = group("architecture-services");
        for label in ["API", "Worker"] {
            assert!(
                services
                    .descendants()
                    .any(|node| node.is_text() && node.text() == Some(label))
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 3, "{fill}");
        assert_eq!(evidence.theme_residual_count(), 0, "{fill}");
    }
}

#[test]
fn architecture_public_render_path_uses_the_immutable_icon_registry() {
    let pack = br##"{
        "prefix":"test",
        "icons":{
            "rocket":{
                "body":"<defs><clipPath id=\"clip\"><path id=\"shape\" d=\"M0 0H16V16H0z\"/></clipPath></defs><path data-icon=\"architecture-registry\" clip-path=\"url(#clip)\" d=\"M0 0H16V16H0z\"/><use href=\"#shape\"/>"
            }
        }
    }"##;
    let registry = IconRegistry::from_packs([IconPack::new(pack)]).expect("valid Iconify pack");
    let source = r#"%%{init: {"architecture": {"numIter": 1, "randomize": false}}}%%
architecture-beta
service api(test:rocket)[API]
service worker(test:rocket)[Worker]
"#;
    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse architecture")
        .expect("architecture detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_icon_registry(registry)
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare architecture");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("architecture-registry".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render architecture")
        .svg()
        .to_owned();

    assert_eq!(
        svg.matches(r#"data-icon="architecture-registry""#).count(),
        2,
        "{svg}"
    );
    assert!(!svg.contains(r#"id="clip""#), "{svg}");
    assert!(!svg.contains(r#"id="shape""#), "{svg}");
    assert!(!svg.contains("url(#clip)"), "{svg}");
    assert!(!svg.contains(r##"href="#shape""##), "{svg}");

    let scoped_ids = Regex::new(r#"id="(IconifyId[^"]+)""#)
        .unwrap()
        .captures_iter(&svg)
        .map(|capture| capture[1].to_string())
        .collect::<Vec<_>>();
    assert!(scoped_ids.len() >= 4, "{svg}");
    assert_eq!(
        scoped_ids.iter().collect::<HashSet<_>>().len(),
        scoped_ids.len(),
        "each Architecture node must receive an independent icon ID scope: {svg}"
    );
}

fn deep_group_chain_diagram(depth: usize) -> String {
    let mut lines = vec![
        r#"%%{init: {"architecture": {"numIter": 1, "randomize": false}}}%%"#.to_string(),
        "architecture-beta".to_string(),
    ];
    for i in 0..depth {
        let parent = if i > 0 {
            format!(" in g{}", i - 1)
        } else {
            Default::default()
        };
        lines.push(format!("  group g{i}(cloud)[G{i}]{parent}"));
    }
    lines.push(format!("  service leaf(server)[Leaf] in g{}", depth - 1));
    lines.join("\n")
}

fn deep_icon_text_diagram(depth: usize) -> String {
    let mut icon_text = String::new();
    for _ in 0..depth {
        icon_text.push_str("<span>");
    }
    icon_text.push_str("Icon");
    for _ in 0..depth {
        icon_text.push_str("</span>");
    }

    format!("architecture-beta\n  service worker \"{icon_text}\" [Worker]\n")
}

fn arrow_transform_after_edge(svg: &str, edge_id: &str) -> String {
    let pattern = format!(r#"id="{}"[^>]*/><polygon([^>]*)>"#, regex::escape(edge_id));
    let re = Regex::new(&pattern).expect("valid regex");
    let attrs = re
        .captures(svg)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or_else(|| panic!("missing arrow polygon after edge {edge_id}"));
    assert!(
        attrs.contains(r#"class="arrow""#),
        "expected polygon after edge {edge_id} to be an arrow, got {attrs}"
    );

    let transform_re = Regex::new(r#"\btransform="([^"]+)""#).expect("valid regex");
    transform_re
        .captures(attrs)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| panic!("missing arrow transform after edge {edge_id}"))
}

fn service_translate(svg: &str, service_id: &str) -> (f64, f64) {
    let pattern = format!(
        r#"id="{}"[^>]*\btransform="translate\(([^,\s]+)[,\s]+([^)]+)\)""#,
        regex::escape(service_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    let caps = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing service transform for {service_id}"));
    let x = caps
        .get(1)
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("invalid service x transform for {service_id}"));
    let y = caps
        .get(2)
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("invalid service y transform for {service_id}"));
    (x, y)
}

fn group_rect(svg: &str, group_id: &str) -> (f64, f64, f64, f64) {
    let pattern = format!(
        r#"id="{}"[^>]*\bx="([^"]+)"[^>]*\by="([^"]+)"[^>]*\bwidth="([^"]+)"[^>]*\bheight="([^"]+)""#,
        regex::escape(group_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    let caps = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing group rect for {group_id}"));
    let parse = |idx: usize, label: &str| {
        caps.get(idx)
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .unwrap_or_else(|| panic!("invalid {label} for {group_id}"))
    };
    (
        parse(1, "x"),
        parse(2, "y"),
        parse(3, "width"),
        parse(4, "height"),
    )
}

fn svg_max_width(svg: &str) -> f64 {
    let re = Regex::new(r#"style="max-width:\s*([^;]+)px;"#).expect("valid regex");
    re.captures(svg)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("missing max-width in root svg style"))
}

fn icon_text_line_clamp(svg: &str, service_id: &str) -> i64 {
    let pattern = format!(
        r#"id="{}"[\s\S]*?-webkit-line-clamp:\s*([0-9]+);"#,
        regex::escape(service_id)
    );
    let re = Regex::new(&pattern).expect("valid regex");
    re.captures(svg)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or_else(|| panic!("missing iconText line clamp for {service_id}"))
}

fn architecture_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Architecture SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect()
}

fn assert_close(actual: f64, expected: f64, message: &str) {
    let delta = (actual - expected).abs();
    assert!(
        delta <= 1e-6,
        "{message}: expected {expected}, got {actual}, delta {delta}"
    );
}

#[test]
fn architecture_svg_handles_deep_group_chain() {
    const DEPTH: usize = 64;
    let engine = MermaidEngine::new();
    for depth in [1, DEPTH] {
        let source = deep_group_chain_diagram(depth);
        let (fixture_parsed_2, fixture_layout_2, fixture_session_2) =
            prepare_architecture_text_with_engine(&engine, &source);
        let options = SvgRenderOptions {
            diagram_id: Some("architecture-deep-groups".to_string()),
            ..Default::default()
        };
        let artifact = family::prepare(&fixture_parsed_2, &fixture_layout_2, fixture_session_2)
            .expect("prepare family fixture");
        let svg = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("architecture-deep-group-svg".to_string())
                .stack_size(DEEP_ARCHITECTURE_RENDER_STACK_SIZE)
                .spawn_scoped(scope, move || {
                    artifact
                        .render_svg(&options, &SvgDebugOptions::default())
                        .expect("render SVG")
                        .svg()
                        .to_owned()
                })
                .expect("spawn architecture deep group SVG test")
                .join()
                .expect("architecture deep group SVG should finish without stack overflow")
        });

        assert!(
            svg.contains(r#"id="architecture-deep-groups-service-leaf""#),
            "expected deepest service to render"
        );
        assert!(
            svg.contains(&format!(
                r#"id="architecture-deep-groups-group-g{}""#,
                depth - 1
            )),
            "expected deepest group to render"
        );
    }
}

#[test]
fn architecture_svg_handles_deep_icon_text_xhtml_fragment() {
    const DEPTH: usize = 1_200;
    for depth in [1, DEPTH] {
        let source = deep_icon_text_diagram(depth);
        let engine = MermaidEngine::new();
        let handle = std::thread::Builder::new()
            .name("architecture-deep-icon-text-svg".to_string())
            .stack_size(DEEP_ARCHITECTURE_RENDER_STACK_SIZE)
            .spawn(move || {
                render_architecture_text_with_engine_and_options(
                    &engine,
                    &source,
                    &SvgRenderOptions {
                        diagram_id: Some("architecture-deep-icon-text".to_string()),
                        ..Default::default()
                    },
                )
            })
            .expect("spawn architecture deep iconText SVG test");
        let svg = handle
            .join()
            .expect("architecture deep iconText SVG should finish without stack overflow");

        assert!(
            svg.contains(r#"id="architecture-deep-icon-text-service-worker""#),
            "expected iconText service to render"
        );
        assert!(
            svg.contains("Icon"),
            "expected deepest iconText label to render"
        );
    }
}

#[test]
fn architecture_service_icon_text_uses_upstream_string_truthiness() {
    let mut no_icon_body = None;
    for (declaration, icon_text, has_icon_text) in [
        ("service api [API]", None, false),
        (r#"service api "" [API]"#, Some(""), false),
        (r#"service api " " [API]"#, Some(" "), true),
        (r#"service api "X" [API]"#, Some("X"), true),
    ] {
        let source = format!("architecture-beta\n{declaration}\n");
        let (parsed, options, session) =
            prepare_architecture_text_with_engine(&MermaidEngine::new(), &source);
        let RenderSemanticModel::Architecture(model) = parsed.model() else {
            panic!("expected Architecture render model");
        };
        assert_eq!(model.nodes.len(), 1);
        assert_eq!(model.nodes[0].icon_text.as_deref(), icon_text);

        let rendered = family::prepare(&parsed, &options, session)
            .expect("prepare Architecture icon-text fixture")
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("architecture-icon-truthiness".to_owned()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render Architecture icon-text fixture");
        let svg = rendered.svg();
        let document = roxmltree::Document::parse(svg).expect("valid Architecture SVG");
        let service = document
            .descendants()
            .find(|node| node.attribute("id") == Some("architecture-icon-truthiness-service-api"))
            .expect("service element");
        let body = service
            .children()
            .rev()
            .find(|node| node.has_tag_name("g"))
            .expect("service body");
        let background = body
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("class") == Some("node-bkg"));
        assert_eq!(background.is_some(), !has_icon_text, "{declaration}");
        assert_eq!(
            body.descendants()
                .filter(|node| node.has_tag_name("foreignObject"))
                .count(),
            usize::from(has_icon_text),
            "{declaration}"
        );
        assert_eq!(
            body.descendants()
                .filter(|node| {
                    node.has_tag_name("rect")
                        && node
                            .attribute("style")
                            .is_some_and(|style| style.contains("fill: #087ebf;"))
                })
                .count(),
            usize::from(has_icon_text),
            "{declaration}"
        );
        if let Some(background) = background {
            assert_eq!(
                background.attribute("id"),
                Some("architecture-icon-truthiness-node-api")
            );
            assert!(background.attribute("style").is_none());
            let body_svg = &svg[body.range()];
            if let Some(expected) = no_icon_body.as_deref() {
                assert_eq!(
                    body_svg, expected,
                    "empty and absent icon bodies must match"
                );
            } else {
                no_icon_body = Some(body_svg.to_owned());
            }
        }
    }
}

#[test]
fn architecture_typed_service_background_matches_emission_and_source_ownership() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    ))
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                    )),
            ),
        )
        .expect("compile Architecture service background theme");
    for (declaration, has_background) in [
        ("service api [API]", true),
        (r#"service api "" [API]"#, true),
        (r#"service api " " [API]"#, false),
        (r#"service api "X" [API]"#, false),
    ] {
        for stroke_owner in [
            None,
            Some("archGroupBorderColor"),
            Some("primaryBorderColor"),
        ] {
            let source_owns_stroke = stroke_owner == Some("archGroupBorderColor");
            let source = format!("architecture-beta\n{declaration}\n");
            let mut config = json!({
                "architecture": {"numIter": 1, "randomize": false},
                "themeVariables": {},
            });
            if let Some(owner) = stroke_owner {
                config["themeVariables"][owner] = json!("#abcdef");
            }
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            );
            let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
                &source,
                &theme,
                engine,
                RenderEnvironment::deterministic(),
            );
            let rendered = family::prepare(&parsed, &options, session)
                .expect("prepare themed Architecture service")
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("render themed Architecture service with complete background evidence");
            let document =
                roxmltree::Document::parse(rendered.svg()).expect("valid Architecture SVG");
            let background = document.descendants().find(|node| {
                node.has_tag_name("path") && node.attribute("class") == Some("node-bkg")
            });
            assert_eq!(background.is_some(), has_background, "{declaration}");
            if let Some(background) = background {
                assert_eq!(background.attribute("id"), Some("architecture-node-api"));
                assert_eq!(
                    background.attribute("style"),
                    Some(if source_owns_stroke {
                        "fill:#123456;"
                    } else {
                        "fill:#123456;stroke:#654321;"
                    }),
                    "{declaration}, stroke owner: {stroke_owner:?}"
                );
            }
            assert_eq!(
                architecture_stylesheet(rendered.svg())
                    .contains("#architecture .node-bkg{fill:none;stroke:#abcdef;"),
                source_owns_stroke,
                "{declaration}, stroke owner: {stroke_owner:?}"
            );

            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let applied = if has_background {
                2 - usize::from(source_owns_stroke)
            } else {
                0
            };
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.accounted_count(), 2);
            assert_eq!(evidence.applied_count(), applied, "{declaration}");
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn architecture_icon_text_typography_evidence_matches_emission_without_titles() {
    let theme = architecture_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Architecture font size"),
    );
    for (declaration, has_icon_text, visible) in [
        ("service api", false, false),
        (r#"service api """#, false, false),
        (r#"service api " ""#, true, false),
        (r#"service api "<br/>""#, true, false),
        (r#"service api "<g>x<b>y</b>z</g>""#, true, false),
        (
            r#"service api "<span style='font-size:0'>X</span>""#,
            true,
            false,
        ),
        (r#"service api "X""#, true, true),
    ] {
        let source = format!("architecture-beta\n{declaration}\n");
        let rendered = try_render_architecture_with_theme(&source, &theme)
            .expect("render Architecture icon-only typography fixture");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Architecture SVG");
        assert_eq!(
            document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            has_icon_text,
            "{declaration}"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(
            evidence.applied_count(),
            usize::from(visible),
            "{declaration}"
        );
        assert_eq!(evidence.not_applicable_count(), usize::from(!visible));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn architecture_icon_text_does_not_claim_svg_text_fill() {
    let theme = architecture_text_fill_theme(CanvasPaint::solid("#123456").unwrap(), None);
    for fragment in [" ", "<br/>", "Visible"] {
        let source = format!("architecture-beta\nservice api \"{fragment}\"\n");
        let rendered = try_render_architecture_with_theme(&source, &theme).unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(
            document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn architecture_svg_text_evidence_uses_final_words_for_every_label_role() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_default(
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Architecture Visible").unwrap())
                            .with_font_size_px(24.0)
                            .unwrap(),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    for body in [
        "service api [$LABEL]\n",
        "group core [$LABEL]\nservice api in core\n",
        "service api\nservice db\napi:R -[$LABEL]- L:db\n",
    ] {
        for (label, visible) in [("---", false), ("<br/>", false), ("Visible", true)] {
            let source = format!("architecture-beta\n{}", body.replace("$LABEL", label));
            let rendered = try_render_architecture_with_theme(&source, &theme).unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let text = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .collect::<Vec<_>>();
            assert_eq!(text.len(), 1, "keep the text branch: {source}");
            assert_eq!(
                text[0].descendants().any(|node| {
                    node.is_text() && node.text().is_some_and(|text| !text.trim().is_empty())
                }),
                visible,
                "{source}",
            );
            if visible {
                assert_eq!(text[0].attribute("style"), Some("fill:#123456;"));
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 3);
            assert_eq!(
                evidence.applied_count(),
                if visible { 3 } else { 0 },
                "{source}"
            );
            assert_eq!(evidence.not_applicable_count(), if visible { 0 } else { 3 });
            assert_eq!(evidence.accounted_count(), 3);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }

    let mixed = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#123456").unwrap())
                        .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                )),
            ),
        )
        .unwrap();
    for label in ["---", "<br/>"] {
        let source = format!("architecture-beta\nservice api [{label}]\n");
        let rendered = try_render_architecture_with_theme(&source, &mixed).unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
    assert!(
        try_render_architecture_with_theme("architecture-beta\nservice api [Visible]\n", &mixed)
            .is_err()
    );
}

#[test]
fn architecture_icon_text_typography_retains_unverified_child_ownership() {
    let theme = architecture_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Architecture Visible").unwrap())
            .with_font_size_px(24.0)
            .unwrap(),
    );
    for (fragment, residuals) in [
        ("<span style='font-family:serif'>X</span>", 1),
        ("<span style='font-size:12px'>X</span>", 1),
        ("<span class='external'>X</span>", 2),
    ] {
        let source = format!("architecture-beta\nservice api \"{fragment}\"\n");
        let error = try_render_architecture_with_theme(&source, &theme)
            .err()
            .expect("child-owned or unknown fonts cannot prove inherited theme consumption");
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::ARCHITECTURE, residuals)),
            "{fragment}"
        );
    }
    let inherited = "architecture-beta\nservice api \"<span>X</span>\"\n";
    let rendered = try_render_architecture_with_theme(inherited, &theme).unwrap();
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn architecture_zero_root_size_keeps_icon_box_and_visible_descendant_family() {
    let theme = architecture_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Architecture Visible").unwrap())
            .with_font_size_px(24.0)
            .unwrap(),
    );
    for (fragment, applied) in [("X", 0), ("<span style='font-size:12px'>X</span>", 1)] {
        let source = format!(
            "---\nconfig: {{themeVariables: {{fontSize: 0}}}}\n---\narchitecture-beta\nservice api \"{fragment}\"\n"
        );
        let rendered = try_render_architecture_with_theme(&source, &theme).unwrap();
        assert!(architecture_stylesheet(rendered.svg()).contains("font-size:0px;"));
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let icon = document
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(icon.attribute("width"), Some("80"));
        assert_eq!(icon.attribute("height"), Some("80"));
        assert!(
            icon.descendants()
                .any(|node| node.attribute("style") == Some("-webkit-line-clamp: Infinity;"))
        );
        assert!(
            icon.descendants()
                .any(|node| node.is_text() && node.text() == Some("X"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), applied, "{fragment}");
        assert_eq!(evidence.not_applicable_count(), 2 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn architecture_unknown_root_size_keeps_independent_icon_text_font_stack() {
    let theme = architecture_typography_theme(
        ThemeTextStyle::default().with_font_stack(FontStack::single("Cousine").unwrap()),
    );
    for (fragment, applied, residuals) in [
        ("<span style='font-size:12px'>X</span>", 1, 0),
        ("<span style='font-size:0px'>X</span>", 0, 0),
        ("<span style='font-size:1em'>X</span>", 0, 1),
        ("X", 0, 1),
    ] {
        for from_source in [false, true] {
            let body = format!("architecture-beta\nservice api \"{fragment}\"\n");
            let config = json!({"themeVariables":{"fontSize":"calc(0px)"}});
            let (source, mut site) = if from_source {
                (format!("---\nconfig: {config}\n---\n{body}"), json!({}))
            } else {
                (body, config)
            };
            site["architecture"] = json!({"numIter":1,"randomize":false});
            let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
                RenderEnvironment::deterministic(),
            );
            let rendered = family::prepare(&parsed, &options, session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
            if residuals != 0 {
                assert_eq!(
                    rendered
                        .err()
                        .expect("unknown visibility remains residual")
                        .unverified_family_theme(),
                    Some((DiagramFamilyId::ARCHITECTURE, residuals)),
                    "{fragment}: source={from_source}"
                );
                continue;
            }
            let rendered = rendered.expect("fixed child visibility proves inherited font stack");
            assert!(architecture_stylesheet(rendered.svg()).contains("font-size:calc(0px);"));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(
                evidence.applied_count(),
                applied,
                "{fragment}: source={from_source}"
            );
            assert_eq!(evidence.not_applicable_count(), 1 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn architecture_svg_honors_mermaid_11_15_style_theme_variables() {
    let engine = MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
        "secure": ["secure", "securityLevel", "startOnLoad", "maxTextSize", "suppressErrorRendering", "maxEdges"]
    })));
    let text = r##"%%{init: {"themeVariables": {"lineColor": "#445566", "primaryBorderColor": "#778899", "archEdgeColor": "#010203", "archEdgeArrowColor": "#040506", "archEdgeWidth": 7, "archGroupBorderColor": "#070809", "archGroupBorderWidth": "6px"}}}%%
architecture-beta
  group core(cloud)[Core]
  service api(server)[API] in core
  service db(database)[DB] in core
  api:R --> L:db
"##;

    let svg = render_architecture_text_with_engine_and_options(
        &engine,
        text,
        &SvgRenderOptions {
            diagram_id: Some("architecture-theme".to_string()),
            ..Default::default()
        },
    );

    assert!(svg.contains(r#"#architecture-theme .edge{stroke-width:7;stroke:#010203;fill:none;}"#));
    assert!(svg.contains(r#"#architecture-theme .arrow{fill:#040506;}"#));
    assert!(svg.contains(
        r#"#architecture-theme .node-bkg{fill:none;stroke:#070809;stroke-width:6px;stroke-dasharray:8;}"#
    ));
    assert!(
        !svg.contains(r#"#architecture-theme .edge{stroke-width:3;stroke:#445566;fill:none;}"#)
    );
    assert!(!svg.contains(r#"#architecture-theme .arrow{fill:#445566;}"#));
    assert!(!svg.contains(
        r#"#architecture-theme .node-bkg{fill:none;stroke:#778899;stroke-width:2px;stroke-dasharray:8;}"#
    ));
}

#[test]
fn architecture_typed_text_fill_reaches_all_text_terminals_for_unqualified_and_default_routes() {
    let source = r#"architecture-beta
  group platform(cloud)[Platform]
  service api(server)[API] in platform
  service db(database)[DB] in platform
  api:R -[request]- L:db
"#;
    for variant in [None, Some(ThemeVariant::Default)] {
        let theme = architecture_text_fill_theme(
            CanvasPaint::solid("#123456").expect("valid Architecture text fill"),
            variant,
        );
        let (fixture_parsed_3, fixture_layout_3, fixture_session_3) =
            prepare_architecture_with_theme_and_environment(
                source,
                &theme,
                MermaidEngine::new(),
                RenderEnvironment::deterministic(),
            );
        let artifact = family::prepare(&fixture_parsed_3, &fixture_layout_3, fixture_session_3)
            .expect("prepare family fixture");
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("architecture-text-fill".to_string()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render themed Architecture text")
            .into_completion();
        let (svg, report) = rendered.into_output_and_report();
        let document = roxmltree::Document::parse(&svg).expect("valid Architecture SVG");
        let text_nodes = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        assert!(
            !text_nodes.is_empty(),
            "Architecture fixture must emit text terminals"
        );
        for label in ["Platform", "API", "DB", "request"] {
            let terminals = text_nodes
                .iter()
                .filter(|node| {
                    node.descendants()
                        .filter_map(|child| child.is_text().then(|| child.text()).flatten())
                        .collect::<String>()
                        .trim()
                        == label
                })
                .collect::<Vec<_>>();
            assert_eq!(
                terminals.len(),
                1,
                "missing or duplicated Architecture label {label}: {svg}"
            );
            assert!(
                terminals
                    .iter()
                    .all(|node| node.attribute("style").is_some_and(|style| {
                        style
                            .split(';')
                            .any(|declaration| declaration == "fill:#123456")
                    })),
                "typed Text.fill must reach {label} for variant {variant:?}: {svg}"
            );
        }
        let evidence = merman_render::__private::family_evidence(&report);
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn architecture_diagonal_arrows_follow_the_actual_edge_segment() {
    let svg = render_architecture_fixture(
        "stress_architecture_batch5_services_outside_groups_crosslinks_078.mmd",
    );

    let diagonal = arrow_transform_after_edge(&svg, "architecture-crosslinks-L_fe_east_api_0");
    assert!(
        diagonal.contains("rotate("),
        "expected diagonal Architecture edge arrow to rotate with the edge segment, got {diagonal}"
    );

    let vertical = arrow_transform_after_edge(&svg, "architecture-crosslinks-L_fe_west_api_0");
    assert!(
        !vertical.contains("rotate("),
        "axis-aligned Architecture arrows should keep the Mermaid-compatible translate-only DOM, got {vertical}"
    );
}

#[test]
fn architecture_group_alignment_follows_source_endpoint_traversal_order() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_deep_nesting_013.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-deep".to_string()),
            ..Default::default()
        },
    );

    let lb = service_translate(&svg, "architecture-deep-service-lb");
    let api = service_translate(&svg, "architecture-deep-service-api");
    let cache = service_translate(&svg, "architecture-deep-service-cache");
    let ext = service_translate(&svg, "architecture-deep-service-ext");

    assert_close(
        lb.1,
        api.1,
        "lb/api should share Mermaid's horizontal alignment",
    );
    assert_close(
        lb.0,
        ext.0,
        "lb/ext should share Mermaid's vertical alignment",
    );
    assert_close(
        api.0,
        cache.0,
        "api/cache should share the final core/data vertical alignment",
    );
}

#[test]
fn architecture_group_rect_uses_configured_padding_for_small_icons() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_batch6_init_fontsize_icon_size_wrap_093.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-padding".to_string()),
            ..Default::default()
        },
    );

    let left = group_rect(&svg, "architecture-padding-group-left");
    assert!(
        left.2.is_finite() && left.2 > 40.0,
        "custom architecture.padding should produce a finite compound width above the icon floor, got width {}",
        left.2
    );
}

#[test]
fn architecture_vertical_edge_label_bounds_use_create_text_y_offsets() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_batch4_init_small_icons_061.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-small-icons".to_string()),
            ..Default::default()
        },
    );

    let group = group_rect(&svg, "architecture-small-icons-group-g");
    assert!(
        group.2 > 158.5 && group.2 < 158.6,
        "small-icon service/group sizing should remain icon-floor dominated, got group width {}",
        group.2
    );
    assert!(
        group.3 > 171.5 && group.3 < 171.6,
        "compound sizing should union expanded body bounds with node-label margin bounds before the parent phase, got group height {}",
        group.3
    );

    let max_width = svg_max_width(&svg);
    assert!(
        max_width.is_finite() && max_width > group.2,
        "vertical edge label createText bbox should contribute beyond the group width, got root width {max_width} and group width {}",
        group.2
    );
}

#[test]
fn architecture_long_title_group_rect_uses_cytoscape_canvas_font_stack() {
    let svg = render_architecture_fixture_with_options(
        "stress_architecture_batch5_long_titles_and_punct_076.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-batch5-long".to_string()),
            ..Default::default()
        },
    );

    let pipeline = group_rect(&svg, "architecture-batch5-long-group-pipeline");
    assert!(
        pipeline.2.is_finite() && pipeline.2 > 400.0,
        "long-title group width should remain dominated by its title content: {}",
        pipeline.2
    );
}

#[test]
fn architecture_layout_caches_service_child_bounds() {
    let text = r#"architecture-beta
  group app(cloud)[Application]
  service gateway(server)[A very long gateway label for group sizing] in app
  service cache(server)[Cache] in app
  gateway:R -- L:cache
"#;
    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Architecture artifact");
    let layout: ArchitectureDiagramLayout = serde_json::from_value(
        artifact.layout_json().expect("serialize layout")["layout"]["ArchitectureDiagram"].clone(),
    )
    .expect("architecture layout projection");
    assert!(
        !layout.cytoscape_service_bounds.is_empty(),
        "expected layout to expose Architecture service child bounds"
    );
}

#[test]
fn architecture_icon_text_clamp_uses_root_svg_font_size_not_architecture_layout_size() {
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(json!({
            "themeVariables": { "fontSize": "24px" },
            "architecture": { "fontSize": 16 }
        })),
    );
    let svg = render_architecture_text_with_engine_and_options(
        &engine,
        include_str!(
            "../../../fixtures/architecture/upstream_architecture_docs_service_icon_text.mmd"
        ),
        &SvgRenderOptions {
            diagram_id: Some("architecture-icontext".to_string()),
            ..Default::default()
        },
    );

    let clamp = icon_text_line_clamp(&svg, "architecture-icontext-service-with_icon_text");
    assert_eq!(
        clamp, 3,
        "iconText clamp must follow the root SVG fontSize=24 rather than architecture.fontSize=16"
    );
}

#[test]
fn architecture_icon_text_clamp_preserves_upstream_computed_font_digits() {
    let body = "architecture-beta\nservice api \"Text Text Text Text\" [Measured]\n";
    let font_stack = FontStack::single("Architecture Clamp").unwrap();
    let expected_font_family = font_stack.as_css();
    for (size, expected_clamp) in [
        (0.0, "Infinity"),
        (0.25, "3"),
        (0.5, "15"),
        (0.75, "1"),
        (1.0, "78"),
        (16.0, "4"),
        (15.5, "0"),
        (80.0, "0"),
    ] {
        for owner in ["site", "source", "typed"] {
            let typed_size = if owner == "typed" {
                // Typed sizes are positive; this value becomes zero in the emitted root CSS.
                if size == 0.0 { 0.0000001 } else { size as f32 }
            } else {
                29.0
            };
            let theme = architecture_typography_theme(
                ThemeTextStyle::default()
                    .with_font_stack(font_stack.clone())
                    .with_font_size_px(typed_size)
                    .unwrap(),
            );
            let mut site = json!({
                "architecture": {"fontSize": 16, "numIter": 1, "randomize": false}
            });
            let font_config = json!({"themeVariables": {"fontSize": format!("{size}px")}});
            let source = match owner {
                "site" => {
                    site["themeVariables"] = font_config["themeVariables"].clone();
                    body.to_owned()
                }
                "source" => format!("---\nconfig: {font_config}\n---\n{body}"),
                "typed" => body.to_owned(),
                _ => unreachable!(),
            };
            let host = Arc::new(CountingArchitectureHost::default());
            let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
                counting_architecture_environment(Arc::clone(&host)),
            );
            let artifact = family::prepare(&parsed, &options, session).unwrap();
            host.styles.lock().unwrap().clear();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            assert!(
                architecture_stylesheet(rendered.svg()).contains(&format!("font-size:{size}px;")),
                "{owner}/{size}: clamp projection must not change the root CSS"
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let clamp = document
                .descendants()
                .filter_map(|node| node.attribute("style"))
                .find(|style| style.starts_with("-webkit-line-clamp:"))
                .expect("Architecture icon-text clamp");
            assert_eq!(
                clamp,
                format!("-webkit-line-clamp: {expected_clamp};"),
                "{owner}/{size}"
            );
            if size > 0.0 {
                // Known-zero glyphs skip measurement; unrelated default-font probes are not
                // measurements of this label. Positive glyphs must use its actual typography.
                let styles = host.styles.lock().unwrap();
                let measured_styles = styles
                    .iter()
                    .filter(|style| {
                        style.font_family.as_deref() == Some(expected_font_family.as_str())
                    })
                    .collect::<Vec<_>>();
                assert!(!measured_styles.is_empty(), "{owner}/{size}");
                assert!(
                    measured_styles.iter().all(|style| style.font_size == size),
                    "{owner}/{size}: glyph measurement must retain the actual size: {measured_styles:?}"
                );
            }
        }
    }
}

#[test]
fn architecture_ignored_base_size_leaves_typed_svg_size_available() {
    let theme =
        architecture_typography_theme(ThemeTextStyle::default().with_font_size_px(29.0).unwrap());
    let body = "architecture-beta\nservice api(server)[API]\n";
    for (value, expected) in [
        (json!(false), 29.0),
        (json!("bad"), 29.0),
        (json!("calc(24px);fill:red"), 29.0),
        (serde_json::Value::Null, 29.0),
        (json!({}), 29.0),
        (json!([]), 29.0),
        (json!(-2), 29.0),
        (json!("-2px"), 29.0),
        (json!(0), 0.0),
        (json!("0px"), 0.0),
        (json!("0em"), 0.0),
        (json!("0%"), 0.0),
        (json!("0rem"), 0.0),
        (json!(0.5), 0.5),
        (json!("0.5px"), 0.5),
        (json!(23), 23.0),
        (json!("23px"), 23.0),
    ] {
        let config = json!({"themeVariables":{"fontSize":value}});
        for from_source in [false, true] {
            let (source, site) = if from_source {
                (format!("---\nconfig: {config}\n---\n{body}"), json!({}))
            } else {
                (body.to_owned(), config.clone())
            };
            let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
                RenderEnvironment::deterministic(),
            );
            let rendered = family::prepare(&parsed, &options, session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            assert!(
                rendered.svg().contains(&format!("font-size:{expected}px;")),
                "source={from_source}: {value}"
            );
            assert!(
                roxmltree::Document::parse(rendered.svg())
                    .unwrap()
                    .descendants()
                    .any(|node| node.has_tag_name("tspan") && node.text() == Some("API"))
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(expected == 29.0));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(expected != 29.0)
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn architecture_ignored_theme_size_does_not_promote_the_root_fallback_to_owner() {
    let theme =
        architecture_typography_theme(ThemeTextStyle::default().with_font_size_px(29.0).unwrap());
    let source = "---\nconfig: {fontSize: 21, themeVariables: {fontSize: -2}}\n---\narchitecture-beta\nservice api [API]\n";
    let rendered = try_render_architecture_with_theme(source, &theme).unwrap();
    assert!(architecture_stylesheet(rendered.svg()).contains("font-size:29px;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
}

#[test]
fn architecture_host_root_sizes_preserve_css_without_certifying_estimates() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_default(
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Cousine").unwrap())
                            .with_font_size_px(29.0)
                            .unwrap(),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    let body = "architecture-beta\nservice api(server)[API]\n";
    for size in [
        "1em",
        "100%",
        "1rem",
        "calc(24px)",
        "var(--audit-size, 24px)",
        "calc(24px);",
        "1em;",
        "100%;",
    ] {
        for from_source in [false, true] {
            if from_source && (size.starts_with("var(") || size.ends_with(';')) {
                // Source admission excludes host substitutions and declaration terminators.
                continue;
            }
            let config = json!({"themeVariables":{"fontSize":size}});
            let (source, mut site) = if from_source {
                (format!("---\nconfig: {config}\n---\n{body}"), json!({}))
            } else {
                (body.to_owned(), config)
            };
            site["architecture"] = json!({"numIter":1, "randomize":false, "fontSize":19});
            let parsed = common::install_parse_compatibility(
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
            )
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
            let host = Arc::new(CountingArchitectureHost::default());
            let session = counting_architecture_environment(Arc::clone(&host))
                .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
                .begin_session_with_theme(&theme)
                .unwrap();
            let rendered =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
            assert!(
                architecture_stylesheet(rendered.svg()).contains(&format!("font-size:{size};"))
            );
            assert!(!architecture_stylesheet(rendered.svg()).contains("font-size:29px;"));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0, "{size}: source={from_source}");
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 2);
            let styles = host.styles.lock().unwrap();
            assert!(styles.iter().any(|style| {
                style.font_family.as_deref() == Some("Helvetica Neue,Helvetica,sans-serif")
                    && style.font_size == 19.0
            }));
        }
    }
}

#[test]
fn architecture_small_svg_sizes_reach_measurement_and_keep_text_dom() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_default(
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("ArchitectureSmall").unwrap())
                            .with_font_size_px(0.5)
                            .unwrap(),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    for configured_size in [None, Some(0.0), Some(0.5)] {
        let prefix = configured_size.map_or_else(String::new, |size| {
            format!("---\nconfig: {{themeVariables: {{fontSize: {size}}}}}\n---\n")
        });
        let source = format!(
            "{prefix}architecture-beta\ngroup core [Group<br/>Second]\nservice api [API<br/>Second] in core\nservice db\napi:R -[Edge<br/>Second]- L:db\n"
        );
        let host = Arc::new(CountingArchitectureHost::default());
        let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
            &source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    json!({"architecture": {"numIter": 1, "randomize": false}}),
                ),
            ),
            counting_architecture_environment(Arc::clone(&host)),
        );
        let artifact = family::prepare(&parsed, &options, session).unwrap();
        host.styles.lock().unwrap().clear();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let expected_size = configured_size.unwrap_or(0.5);
        assert!(
            architecture_stylesheet(rendered.svg())
                .contains(&format!("font-size:{expected_size}px;"))
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .count(),
            3
        );
        for word in ["Group", "API", "Edge"] {
            assert!(
                document
                    .descendants()
                    .any(|node| node.is_text() && node.text() == Some(word))
            );
        }
        let svg_styles = host.styles.lock().unwrap();
        assert!(
            svg_styles
                .iter()
                .filter(|style| style.font_family.as_deref() == Some("ArchitectureSmall"))
                .all(|style| style.font_size == expected_size)
        );
        if expected_size > 0.0 {
            assert!(
                svg_styles
                    .iter()
                    .any(|style| style.font_family.as_deref() == Some("ArchitectureSmall"))
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        let applied = if expected_size == 0.0 {
            0
        } else if configured_size.is_some() {
            2
        } else {
            3
        };
        assert_eq!(evidence.required_count(), 3);
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), 3 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn architecture_small_service_text_rows_use_the_emitted_em_step() {
    let theme =
        architecture_typography_theme(ThemeTextStyle::default().with_font_size_px(0.5).unwrap());
    for (size, expected_delta) in [(0.0, 0.0), (0.5, 0.55)] {
        let render_height = |label: &str| {
            let source = format!(
                "---\nconfig: {{themeVariables: {{fontSize: {size}}}}}\n---\narchitecture-beta\nservice api [{label}]\n"
            );
            let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
                &source,
                &theme,
                MermaidEngine::new(),
                counting_architecture_environment(Arc::new(CountingArchitectureHost::default())),
            );
            let rendered = family::prepare(&parsed, &options, session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            roxmltree::Document::parse(rendered.svg())
                .unwrap()
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .nth(3)
                .unwrap()
                .parse::<f64>()
                .unwrap()
        };
        assert_close(
            render_height("A<br/>B") - render_height("A"),
            expected_delta,
            "zero text has no glyph extent; 0.5px rows advance by 0.55px",
        );
    }
}

#[test]
fn architecture_typed_base_typography_reaches_svg_css_and_preserves_cytoscape_layout_owner() {
    let font_stack =
        FontStack::new(["Architecture Typed", "monospace"]).expect("valid Architecture font");
    let expected_font_family = font_stack.as_css();
    let theme = architecture_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(font_stack)
            .with_font_size_px(24.0)
            .expect("valid Architecture font size"),
    );
    let host = Arc::new(CountingArchitectureHost::default());
    let source = r#"architecture-beta
  group platform(cloud)[Platform]
  service api(server)[API] in platform
  service icon_text "Typed icon text" [Icon text title]
  api:R -[request]- L:icon_text
"#;
    let (fixture_parsed_4, fixture_layout_4, fixture_session_4) =
        prepare_architecture_with_theme_and_environment(
            source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(json!({
                    "architecture": { "fontSize": 19 }
                })),
            ),
            counting_architecture_environment(Arc::clone(&host)),
        );
    let artifact = family::prepare(&fixture_parsed_4, &fixture_layout_4, fixture_session_4)
        .expect("prepare family fixture");

    let layout_styles = host
        .styles
        .lock()
        .expect("Architecture layout styles lock")
        .clone();
    assert!(
        layout_styles.iter().any(|style| {
            style.font_family.as_deref() == Some("Helvetica Neue,Helvetica,sans-serif")
                && style.font_size == 19.0
        }),
        "Cytoscape layout must keep architecture.fontSize and its fixed font owner: {layout_styles:?}"
    );
    assert!(
        layout_styles.iter().all(|style| {
            style.font_family.as_deref() != Some(expected_font_family.as_str())
                || style.font_size != 24.0
        }),
        "typed SVG typography must not leak into Cytoscape layout measurements: {layout_styles:?}"
    );

    host.calls.store(0, Ordering::Relaxed);
    host.operations
        .lock()
        .expect("Architecture host operations lock")
        .clear();
    host.styles
        .lock()
        .expect("Architecture host styles lock")
        .clear();

    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("architecture-typed".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Architecture");
    let stylesheet = architecture_stylesheet(rendered.svg());
    assert!(
        stylesheet.contains("Architecture Typed")
            && stylesheet.contains("monospace")
            && stylesheet.contains("#architecture-typed{font-family:")
            && stylesheet.contains("font-size:24px"),
        "typed Architecture typography must reach the final stylesheet: {stylesheet}"
    );
    assert_eq!(
        icon_text_line_clamp(rendered.svg(), "architecture-typed-service-icon_text"),
        3,
        "iconText must inherit the typed root SVG font size"
    );

    let render_styles = host
        .styles
        .lock()
        .expect("Architecture render styles lock")
        .clone();
    assert!(
        render_styles.iter().any(|style| {
            style.font_family.as_deref() == Some(expected_font_family.as_str())
                && style.font_size == 24.0
        }),
        "final SVG text measurements must use the typed base typography: {render_styles:?}"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn architecture_near_integer_font_size_uses_one_canonical_css_value() {
    let font_stack =
        FontStack::new(["Architecture Canonical", "monospace"]).expect("valid Architecture font");
    let theme = architecture_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(font_stack)
            // This is the largest f32 below 16.0. CSS emission canonicalizes it to 16px.
            .with_font_size_px(f32::from_bits(0x417f_ffff))
            .expect("valid Architecture font size"),
    );
    let host = Arc::new(CountingArchitectureHost::default());
    let source = r#"architecture-beta
  group platform(cloud)[Platform]
  service api(server)[API] in platform
  service icon_text "Canonical icon text" [Icon text title]
  api:R -[request]- L:icon_text
"#;
    let (fixture_parsed_5, fixture_layout_5, fixture_session_5) =
        prepare_architecture_with_theme_and_environment(
            source,
            &theme,
            MermaidEngine::new(),
            counting_architecture_environment(Arc::clone(&host)),
        );
    let artifact = family::prepare(&fixture_parsed_5, &fixture_layout_5, fixture_session_5)
        .expect("prepare family fixture");

    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("architecture-canonical-size".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Architecture");
    let stylesheet = architecture_stylesheet(rendered.svg());
    assert!(
        stylesheet.contains("font-size:16px")
            && !stylesheet.contains("15.999999")
            && !stylesheet.contains("15.999998"),
        "near-integer typed font size must use the canonical CSS spelling: {stylesheet}"
    );

    let render_styles = host
        .styles
        .lock()
        .expect("Architecture render styles lock")
        .clone();
    assert!(
        render_styles.iter().any(|style| style.font_size == 16.0),
        "text measurement must consume the same canonical font size as CSS: {render_styles:?}"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn architecture_svg_uses_the_session_measurement_route() {
    let host = Arc::new(CountingArchitectureHost::default());
    let session = counting_architecture_environment(Arc::clone(&host))
        .begin_session()
        .expect("begin render session");
    let source = r#"architecture-beta
  group app(cloud)[Application platform]
  service api(server)[API service] in app
  service db(database)[Data store] in app
  service outside(server)[Outside service]
  api:R -[request path]- L:db
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let layout_options = LayoutOptions::headless_svg_defaults();
    let artifact =
        family::prepare(&parsed, &layout_options, session).expect("prepare Architecture artifact");
    host.calls.store(0, Ordering::Relaxed);
    host.operations
        .lock()
        .expect("Architecture host operations lock")
        .clear();
    host.reject_generic_svg_measurement
        .store(true, Ordering::Relaxed);

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Architecture artifact");
    let completion = rendered.into_completion();
    assert_eq!(
        completion.report().family_id(),
        DiagramFamilyId::ARCHITECTURE
    );
    let (host_svg, family_report) = completion.into_output_and_report();

    assert!(
        host.calls.load(Ordering::Relaxed) > 0,
        "Architecture must not bypass the session with a family-local deterministic measurer"
    );
    let operations = host
        .operations
        .lock()
        .expect("Architecture host operations lock");
    assert!(
        operations.contains(&TextMeasurementOperation::ComputedLength),
        "Architecture createText wrapping must request SVG computed text length"
    );
    assert!(
        operations.contains(&TextMeasurementOperation::BBoxX),
        "Architecture root bounds must request the emitted formatted-text bbox extents"
    );
    assert!(
        operations.contains(&TextMeasurementOperation::TspanBBoxHeight),
        "Architecture root bounds must request the rendered tspan height"
    );
    assert!(
        operations.contains(&TextMeasurementOperation::CreateTextMiddleBBoxYOffset),
        "Architecture root bounds must request the inherited middle-baseline bbox y offset"
    );
    drop(operations);
    assert!(
        family_report
            .session_report()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                entry.provenance().operation == TextMeasurementOperation::ComputedLength
                    && entry.provenance().phase == TextMeasurementPhase::ComputedLength
                    && entry.provenance().source
                        == merman_render::environment::TextMeasurementSource::Host
            }),
        "Architecture wrap probes must retain computed-length host provenance"
    );
    assert!(
        family_report
            .session_report()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                entry.provenance().operation == TextMeasurementOperation::BBoxX
                    && entry.provenance().phase == TextMeasurementPhase::SvgBBox
                    && entry.provenance().source
                        == merman_render::environment::TextMeasurementSource::Host
            }),
        "Architecture SVG bbox measurements must retain the host phase provenance"
    );

    let parity_session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parity_artifact = family::prepare(&parsed, &layout_options, parity_session)
        .expect("prepare parity Architecture artifact");
    let parity_rendered = parity_artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render parity Architecture artifact");
    let parity_completion = parity_rendered.into_completion();
    assert_eq!(
        parity_completion.report().family_id(),
        DiagramFamilyId::ARCHITECTURE
    );
    let parity_svg = parity_completion.into_output_and_report().0;
    assert_ne!(
        host_svg, parity_svg,
        "host metrics must change observable geometry"
    );
}

#[test]
fn architecture_zero_seed_consumes_the_operation_stream_without_rerun_reset() {
    fn render_with_seed(source: &str, ambient_seed: u64) -> (ArchitectureDiagramLayout, String) {
        let session = RenderEnvironment::deterministic()
            .with_runtime_policy(
                merman_model::runtime::RuntimePolicy::deterministic().with_fixed_seed(ambient_seed),
            )
            .begin_session()
            .expect("begin render session");
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse ok")
            .expect("diagram detected")
            .into_presentation();
        let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .expect("prepare Architecture artifact");
        let layout: ArchitectureDiagramLayout = serde_json::from_value(
            artifact.layout_json().expect("serialize layout")["layout"]["ArchitectureDiagram"]
                .clone(),
        )
        .expect("architecture layout projection");
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("architecture-session-seed".to_string()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render Architecture artifact");
        let completion = rendered.into_completion();
        assert_eq!(
            completion.report().family_id(),
            DiagramFamilyId::ARCHITECTURE
        );
        let svg = completion.into_output_and_report().0;
        (layout, svg)
    }

    let zero = r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7, "architecture": {"seed": 0}}}%%
architecture-beta
  service api(server)[API]
  service db(database)[Database]
  api:R --> L:db
"#;
    let (zero_layout, zero_svg) = render_with_seed(zero, 77);
    let (repeated_zero_layout, repeated_zero_svg) = render_with_seed(zero, 77);

    assert_eq!(
        serde_json::to_value(zero_layout).unwrap(),
        serde_json::to_value(repeated_zero_layout).unwrap(),
        "a pinned operation stream must remain reproducible across operations"
    );
    assert_eq!(
        zero_svg, repeated_zero_svg,
        "a fixed handDrawnSeed and pinned operation stream must keep SVG reproducible"
    );
}

#[test]
#[ignore = "diagnostic matrix for Architecture root-width experiments"]
fn architecture_root_width_diagnostic_matrix() {
    let fixtures = [
        "stress_architecture_batch5_long_titles_and_punct_076.mmd",
        "stress_architecture_batch4_init_small_icons_061.mmd",
        "stress_architecture_html_titles_and_escapes_041.mmd",
        "stress_architecture_unicode_and_xml_escapes_019.mmd",
        "stress_architecture_long_group_titles_018.mmd",
        "stress_architecture_batch6_long_group_titles_wrapping_extreme_095.mmd",
    ];

    for fixture in fixtures {
        let svg = render_architecture_fixture_with_options(
            fixture,
            &SvgRenderOptions {
                diagram_id: Some("architecture-diagnostic".to_string()),
                ..Default::default()
            },
        );
        let max_width = svg_max_width(&svg);
        println!("{fixture}: max-width={max_width}");
    }
}

#[test]
fn repair_u7_architecture_source_owned_stroke_suppresses_unsupported_stroke_only() {
    use merman_render::diagram_theme::{
        GradientStop, LinearGradient, OrdinalSelector, ThemeColorValue,
    };
    let gradient = CanvasPaint::LinearGradient(
        LinearGradient::new(
            0.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#ff0000").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#0000ff").unwrap()).unwrap(),
            ],
        )
        .unwrap(),
    );
    let source =
        "architecture-beta\nservice api(server)[API]\nservice db(database)[DB]\napi:R --> L:db";
    for owner in ["archEdgeColor", "lineColor"] {
        for ordinal in [false, true] {
            let paint = if ordinal {
                CanvasPaint::solid("#123456").unwrap()
            } else {
                gradient.clone()
            };
            let rule = ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default().with_stroke(paint),
            );
            let rule = if ordinal {
                rule.with_ordinal(OrdinalSelector::Exact(1))
            } else {
                rule
            };
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)),
                )
                .unwrap();
            let config = json!({"themeVariables":{owner:"#abcdef"}});
            let (fixture_parsed_6, fixture_layout_6, fixture_session_6) =
                prepare_architecture_with_theme_and_environment(
                    source,
                    &theme,
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                    RenderEnvironment::deterministic(),
                );
            let artifact = family::prepare(&fixture_parsed_6, &fixture_layout_6, fixture_session_6)
                .expect("prepare family fixture");
            let rendered = artifact
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some("owned-edge".into()),
                        ..Default::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap();
            let stylesheet = architecture_stylesheet(rendered.svg());
            let edge_stroke = stylesheet
                .split('}')
                .filter_map(|rule| rule.split_once('{'))
                .find(|(selector, _)| selector.trim().ends_with(".edge"))
                .and_then(|(_, declarations)| {
                    declarations
                        .split(';')
                        .rev()
                        .filter_map(|declaration| declaration.split_once(':'))
                        .find_map(|(property, value)| {
                            (property.trim() == "stroke").then_some(value.trim())
                        })
                });
            assert_eq!(edge_stroke, Some("#abcdef"));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let edges = document
                .descendants()
                .filter(|node| node.has_tag_name("path") && node.attribute("class") == Some("edge"))
                .collect::<Vec<_>>();
            assert!(!edges.is_empty());
            assert!(edges.iter().all(|node| node.attribute("style").is_none()));
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn architecture_without_theme_measures_service_titles_only_for_the_viewport() {
    let source = "architecture-beta\nservice api(server)[API]";
    let host = Arc::new(CountingArchitectureHost::default());
    let environment = counting_architecture_environment(Arc::clone(&host));
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        environment.begin_session().unwrap(),
    )
    .unwrap();
    host.operations.lock().unwrap().clear();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("unobserved-service".into()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    let operations = host.operations.lock().unwrap();
    for operation in [
        TextMeasurementOperation::BBoxX,
        TextMeasurementOperation::TspanBBoxHeight,
        TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
    ] {
        assert_eq!(
            operations
                .iter()
                .filter(|actual| **actual == operation)
                .count(),
            1,
            "only the retained viewport planning observes {operation:?}: {operations:?}"
        );
    }
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("tspan") && node.text() == Some("API"))
    );
}

#[test]
#[cfg(not(merman_internal_theme_acceptance))]
fn architecture_requested_text_style_does_not_repeat_viewport_measurements() {
    let source = "architecture-beta\nservice api(server)[API]";
    let themes = [
        architecture_text_fill_theme(CanvasPaint::solid("#123456").unwrap(), None),
        architecture_typography_theme(
            ThemeTextStyle::default()
                .with_font_stack(FontStack::single("Architecture Typed").unwrap())
                .with_font_size_px(24.0)
                .unwrap(),
        ),
    ];
    for theme in themes {
        let host = Arc::new(CountingArchitectureHost::default());
        let (fixture_parsed_7, fixture_layout_7, fixture_session_7) =
            prepare_architecture_with_theme_and_environment(
                source,
                &theme,
                MermaidEngine::new(),
                counting_architecture_environment(Arc::clone(&host)),
            );
        let artifact = family::prepare(&fixture_parsed_7, &fixture_layout_7, fixture_session_7)
            .expect("prepare family fixture");
        host.operations.lock().unwrap().clear();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let operations = host.operations.lock().unwrap();
        for operation in [
            TextMeasurementOperation::BBoxX,
            TextMeasurementOperation::TspanBBoxHeight,
            TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
        ] {
            assert_eq!(
                operations
                    .iter()
                    .filter(|actual| **actual == operation)
                    .count(),
                1,
                "the final viewport owns the only {operation:?} measurement: {operations:?}",
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert!(evidence.applied_count() > 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn architecture_edge_and_marker_ownership_follows_selected_palette_paths() {
    let source =
        "architecture-beta\nservice api(server)[API]\nservice db(database)[DB]\napi:R --> L:db";
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#995511").unwrap()),
                    ))
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#446688").unwrap()),
                    )),
            ),
        )
        .unwrap();
    for (baseline, variables, edge_applies_by_owner, marker_applies_by_owner) in [
        (
            "default",
            json!({"archEdgeColor": "#333333"}),
            [false, false],
            [true, true],
        ),
        (
            "default",
            json!({"archEdgeColor": "#abcdef"}),
            [false, false],
            [true, true],
        ),
        (
            "default",
            json!({"archEdgeColor": "#333333", "archEdgeArrowColor": "#333333"}),
            [false, false],
            [false, false],
        ),
        (
            "default",
            json!({"lineColor": "#333333"}),
            [false, true],
            [false, true],
        ),
        (
            "base",
            json!({"lineColor": "#777"}),
            [true, true],
            [true, true],
        ),
        (
            "base",
            json!({"lineColor": "#123456"}),
            [true, true],
            [true, true],
        ),
        (
            "base",
            json!({"archEdgeColor": "#777"}),
            [false, false],
            [true, true],
        ),
        (
            "base",
            json!({"archEdgeArrowColor": "#777"}),
            [true, true],
            [false, false],
        ),
        (
            "dark",
            json!({"lineColor": "lightgrey"}),
            [true, true],
            [true, true],
        ),
    ] {
        for source_owned in [false, true] {
            let edge_applies = edge_applies_by_owner[usize::from(source_owned)];
            let marker_applies = marker_applies_by_owner[usize::from(source_owned)];
            let config = json!({"themeVariables": variables});
            let mut site = json!({"theme": baseline});
            let source = if source_owned {
                format!("---\nconfig: {config}\n---\n{source}")
            } else {
                site["themeVariables"] = variables.clone();
                source.to_owned()
            };
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(site),
            );
            let (parsed, options, session) = prepare_architecture_with_theme_and_environment(
                &source,
                &theme,
                engine,
                RenderEnvironment::deterministic(),
            );
            let rendered = family::prepare(&parsed, &options, session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let arrow = document
                .descendants()
                .find(|node| node.attribute("class") == Some("arrow"))
                .unwrap();
            assert_eq!(
                arrow.attribute("style"),
                marker_applies.then_some("fill:#446688;"),
                "baseline={baseline} source_owned={source_owned} variables={variables}"
            );
            let edge = document
                .descendants()
                .find(|node| node.has_tag_name("path") && node.attribute("class") == Some("edge"))
                .unwrap();
            assert_eq!(
                edge.attribute("style"),
                edge_applies.then_some("stroke:#995511;"),
                "baseline={baseline} source_owned={source_owned} variables={variables}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2);
            let applied = usize::from(edge_applies) + usize::from(marker_applies);
            assert_eq!(evidence.applied_count(), applied);
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn architecture_group_border_keeps_constructor_color_and_property_ownership() {
    let diagram =
        "architecture-beta\ngroup backend(cloud)[Backend]\nservice api(server)[API] in backend";
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#995511").unwrap()),
            ))),
        )
        .unwrap();
    for (baseline, constructor_color) in [
        ("default", "hsl(240, 60%, 86.2745098039%)"),
        ("base", "#000"),
        ("dark", "#cccccc"),
    ] {
        for primary_border in [constructor_color, "#123456"] {
            for explicit_group in [false, true] {
                for from_source in [false, true] {
                    let mut variables = json!({"primaryBorderColor": primary_border});
                    if explicit_group {
                        variables["archGroupBorderColor"] = json!("#556677");
                    }
                    let mut site = json!({"theme": baseline});
                    let source = if from_source {
                        format!(
                            "---\nconfig: {}\n---\n{diagram}",
                            json!({"themeVariables": variables})
                        )
                    } else {
                        site["themeVariables"] = variables.clone();
                        diagram.to_owned()
                    };
                    let (parsed, options, session) =
                        prepare_architecture_with_theme_and_environment(
                            &source,
                            &theme,
                            MermaidEngine::new().with_site_config(
                                merman_core::presentation::SourcePresentationConfig::from_value(
                                    site,
                                ),
                            ),
                            RenderEnvironment::deterministic(),
                        );
                    let rendered = family::prepare(&parsed, &options, session)
                        .unwrap()
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                        .unwrap();
                    assert_eq!(
                        rendered
                            .metadata()
                            .effective_config
                            .get_str("themeVariables.archGroupBorderColor"),
                        Some(if explicit_group {
                            "#556677"
                        } else {
                            constructor_color
                        }),
                        "{baseline}: primary={primary_border}, explicit={explicit_group}, source={from_source}"
                    );
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    let group = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("rect")
                                && node
                                    .attribute("id")
                                    .is_some_and(|id| id.ends_with("-group-backend"))
                        })
                        .unwrap();
                    assert_eq!(
                        group.attribute("style"),
                        (!explicit_group).then_some("stroke:#995511;"),
                        "{baseline}: primary={primary_border}, explicit={explicit_group}, source={from_source}"
                    );
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(evidence.required_count(), 1);
                    assert_eq!(evidence.applied_count(), usize::from(!explicit_group));
                    assert_eq!(evidence.not_applicable_count(), usize::from(explicit_group));
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn repair_u7_architecture_source_stroke_keeps_other_property_residuals() {
    let source =
        "architecture-beta\nservice api(server)[API]\nservice db(database)[DB]\napi:R --> L:db";
    let patch = ThemeStylePatch::default()
        .with_stroke(CanvasPaint::solid("#123456").unwrap())
        .with_fill(CanvasPaint::solid("#654321").unwrap());
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, patch)),
        ))
        .unwrap();
    let parsed = common::install_parse_compatibility(
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                json!({"themeVariables":{"archEdgeColor":"#abcdef"}}),
            ),
        ),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .unwrap()
    .unwrap()
    .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .unwrap();
    let prepared = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session);
    let result = prepared.and_then(|artifact| {
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    });
    assert!(
        result.is_err(),
        "the source-owned stroke cannot certify an unsupported Edge.fill sibling"
    );
}

#[allow(dead_code)]
mod common;

#[test]
fn architecture_svg_preserves_db_accessibility_text() {
    for (title, description, expected_title, expected_description) in [
        (
            "Accessible <!--a11y-->",
            "First\n<!--a11y-->",
            Some("Accessible "),
            Some("First\n"),
        ),
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            Some("\u{0085}"),
            Some("\u{0085}"),
        ),
        ("<!--a11y-->", "<!--a11y-->", None, None),
    ] {
        let source = format!(
            "architecture-beta\naccTitle: {title}\naccDescr {{\n{description}\n}}\nservice app(server)[App]\n"
        );
        let svg = render_architecture_text_with_options(&source, &SvgRenderOptions::default());
        common::assert_svg_accessibility(&svg, expected_title, expected_description);
    }
}
