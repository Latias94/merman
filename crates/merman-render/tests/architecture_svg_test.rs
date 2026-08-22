#![cfg(feature = "layout-cytoscape")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, GradientStop,
    LinearGradient, OrdinalSelector, PatternKind, PatternSpec, RadialGradient, Specified,
    ThemeColorValue, ThemeLength, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::ArchitectureDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{IconPack, IconRegistry, SvgDebugOptions, SvgRenderOptions};
use merman_render::text::TextMetrics;
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const TWO_ARCHITECTURE_EDGES: &str = r#"architecture-beta
  service api(server)[API]
  service worker(server)[Worker]
  service db(database)[DB]
  api:R --> L:worker
  worker:R --> L:db
"#;

const ARCHITECTURE_WITHOUT_EDGES: &str = r#"architecture-beta
  service api(server)[API]
"#;

#[derive(Default)]
struct CountingArchitectureHost {
    calls: AtomicUsize,
    operations: Mutex<Vec<TextMeasurementOperation>>,
    reject_generic_svg_measurement: std::sync::atomic::AtomicBool,
}

impl HostTextMeasurer for CountingArchitectureHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.operations
            .lock()
            .expect("Architecture host operations lock")
            .push(request.operation);
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
    let engine = Engine::new();
    render_architecture_text_with_engine_and_options(&engine, text, options)
}

fn render_architecture_text_with_engine_and_options(
    engine: &Engine,
    text: &str,
    options: &SvgRenderOptions,
) -> String {
    prepare_architecture_text_with_engine(engine, text)
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned()
}

fn prepare_architecture_text_with_engine(
    engine: &Engine,
    text: &str,
) -> family::FamilyRenderArtifact {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions::headless_svg_defaults();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    family::prepare(parsed, &layout_options, session).expect("layout ok")
}

fn architecture_edge_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Architecture Edge rules")
}

fn architecture_edge_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    architecture_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(stroke),
    )
    .for_family(DiagramFamilyId::ARCHITECTURE)])
}

fn architecture_terminal_surface_theme() -> DiagramTheme {
    architecture_edge_rules_theme([
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").expect("valid Architecture service fill")),
        )
        .for_family(DiagramFamilyId::ARCHITECTURE),
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid("#234567").expect("valid Architecture service stroke"),
            ),
        )
        .for_family(DiagramFamilyId::ARCHITECTURE),
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#345678").expect("valid Architecture text fill")),
        )
        .for_family(DiagramFamilyId::ARCHITECTURE),
        ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#456789").expect("valid Architecture arrow fill")),
        )
        .for_family(DiagramFamilyId::ARCHITECTURE),
    ])
}

fn try_render_architecture_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Architecture")
        .expect("detect themed Architecture");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Architecture session");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn render_architecture_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
) -> family::RenderedFamilySvg {
    try_render_architecture_with_theme_and_engine(source, theme, engine, diagram_id)
        .expect("render themed Architecture")
}

fn has_class(node: &roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn architecture_edge_paths<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("path") && has_class(node, "edge"))
        .collect()
}

fn architecture_arrow_polygons<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("polygon") && has_class(node, "arrow"))
        .collect()
}

fn stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Architecture SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect()
}

fn stylesheet_property<'a>(css: &'a str, selector: &str, property: &str) -> Option<&'a str> {
    css.split('}').find_map(|rule| {
        let (candidate, declarations) = rule.split_once('{')?;
        if candidate.trim() != selector {
            return None;
        }
        declarations.split(';').find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            (name.trim() == property).then_some(value.trim())
        })
    })
}

fn try_render_architecture_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Architecture resource-bound session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Architecture resource-bound fixture")
        .expect("detect Architecture resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("architecture-bounded".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
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
fn architecture_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"%%{init: {"architecture": {"numIter": 1, "randomize": false}}}%%
architecture-beta
  service api(server)[API]
  service db(database)[Database]
  api:R --> L:db
"#;
    let baseline = try_render_architecture_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Architecture baseline");
    let exact_bytes = baseline.len();
    assert!(
        exact_bytes > 1,
        "Architecture fixture must emit a non-empty SVG"
    );

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Architecture SVG byte ceiling");
    let exact = try_render_architecture_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Architecture family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Architecture SVG byte ceiling");
    let error = try_render_architecture_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Architecture family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Architecture MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
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
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse architecture")
        .expect("architecture detected");
    let session = RenderEnvironment::deterministic()
        .with_icon_registry(registry)
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
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
    let engine = Engine::new();
    for depth in [1, DEPTH] {
        let source = deep_group_chain_diagram(depth);
        let artifact = prepare_architecture_text_with_engine(&engine, &source);
        let options = SvgRenderOptions {
            diagram_id: Some("architecture-deep-groups".to_string()),
            ..Default::default()
        };
        let handle = std::thread::Builder::new()
            .name("architecture-deep-group-svg".to_string())
            .stack_size(128 * 1024)
            .spawn(move || {
                artifact
                    .render_svg(&options, &SvgDebugOptions::default())
                    .expect("render SVG")
                    .svg()
                    .to_owned()
            })
            .expect("spawn architecture deep group SVG test");
        let svg = handle
            .join()
            .expect("architecture deep group SVG should finish without stack overflow");

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
    let source = deep_icon_text_diagram(DEPTH);
    let engine = Engine::new();
    let handle = std::thread::Builder::new()
        .name("architecture-deep-icon-text-svg".to_string())
        .stack_size(128 * 1024)
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

#[test]
fn architecture_svg_honors_mermaid_11_15_style_theme_variables() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
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
fn architecture_edge_stroke_reaches_every_edge_segment_without_recoloring_arrows() {
    for (case, stroke, expected_stroke) in [
        (
            "solid",
            CanvasPaint::solid("#123456").expect("valid Architecture Edge stroke"),
            "#123456",
        ),
        ("transparent", CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = architecture_edge_stroke_theme(stroke);
        let rendered = render_architecture_with_theme_and_engine(
            TWO_ARCHITECTURE_EDGES,
            &theme,
            Engine::new(),
            &format!("architecture-edge-{case}"),
        );
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid Architecture Edge SVG");
        let edges = architecture_edge_paths(&document);
        let arrows = architecture_arrow_polygons(&document);

        assert_eq!(edges.len(), 2, "{case}");
        assert!(
            edges.iter().all(|edge| edge.attribute("style")
                == Some(format!("stroke:{expected_stroke};").as_str())),
            "{case}: every actual Architecture edge path must carry the direct terminal stroke",
        );
        assert_eq!(arrows.len(), 2, "{case}");
        assert!(
            arrows.iter().all(|arrow| arrow
                .attribute("style")
                .is_none_or(|style| !style.contains(expected_stroke))),
            "{case}: Architecture .arrow fill remains owned by archEdgeArrowColor",
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{case}");
        assert_eq!(evidence.applied_count(), 1, "{case}");
        assert_eq!(evidence.not_applicable_count(), 0, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn architecture_service_text_and_arrow_surfaces_have_independent_terminal_owners() {
    let source = r#"architecture-beta
  group core(cloud)[Core]
  service api[API] in core
  service worker[Worker] in core
  service db[Database] in core
  api:R --> L:worker
"#;
    let rendered = render_architecture_with_theme_and_engine(
        source,
        &architecture_terminal_surface_theme(),
        Engine::new(),
        "architecture-surfaces",
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Architecture surface SVG");

    let service_backgrounds = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && has_class(node, "node-bkg")
                && node.attribute("id").is_some_and(|id| id.contains("-node-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(service_backgrounds.len(), 3, "{}", rendered.svg());
    assert!(
        service_backgrounds
            .iter()
            .all(|node| { node.attribute("style") == Some("fill:#123456;stroke:#234567;") })
    );

    let group_background = document
        .descendants()
        .find(|node| node.attribute("id") == Some("architecture-surfaces-group-core"))
        .expect("Architecture group background");
    assert!(
        group_background
            .attribute("style")
            .is_none_or(|style| !style.contains("#123456") && !style.contains("#234567")),
        "service paint must not recolor the independent Cluster surface"
    );

    let visible_svg_text = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect::<Vec<_>>();
    assert_eq!(visible_svg_text.len(), 4, "{}", rendered.svg());
    assert!(
        visible_svg_text
            .iter()
            .all(|text| { text.attribute("style") == Some("fill:#345678;") })
    );
    assert!(
        visible_svg_text
            .iter()
            .all(|text| text.attribute("data-merman-text-bbox").is_none()),
        "writer-owned terminal bounds must stay in the production receipt, not the stable SVG DOM"
    );

    let arrows = architecture_arrow_polygons(&document);
    assert_eq!(arrows.len(), 1, "{}", rendered.svg());
    assert_eq!(arrows[0].attribute("style"), Some("fill:#456789;"));
    let edges = architecture_edge_paths(&document);
    assert_eq!(edges.len(), 1, "{}", rendered.svg());
    assert!(
        edges[0]
            .attribute("style")
            .is_none_or(|style| !style.contains("#456789")),
        "Marker.fill must not replace the independent Edge.stroke terminal"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 4);
    assert_eq!(evidence.applied_count(), 4);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn architecture_icon_text_keeps_its_fixed_mermaid_owner_outside_typed_text() {
    let source = r#"architecture-beta
  service worker "Icon" [Worker]
"#;
    let rendered = render_architecture_with_theme_and_engine(
        source,
        &architecture_terminal_surface_theme(),
        Engine::new(),
        "architecture-icon-text-owner",
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Architecture icon-text SVG");

    let icon_text = document
        .descendants()
        .find(|node| node.has_tag_name("div") && has_class(node, "node-icon-text"))
        .expect("Architecture icon-text container");
    let icon_text_content = icon_text
        .children()
        .find(|node| node.has_tag_name("div"))
        .expect("Architecture icon-text content");
    assert!(
        icon_text_content
            .attribute("style")
            .is_some_and(|style| !style.contains("color:")),
        "typed Text.fill must not override Mermaid's fixed white iconText owner"
    );
    assert_eq!(
        stylesheet_property(
            &stylesheet(rendered.svg()),
            "#architecture-icon-text-owner .node-icon-text>div",
            "color",
        ),
        Some("#fff")
    );

    let service = document
        .descendants()
        .find(|node| node.attribute("id") == Some("architecture-icon-text-owner-service-worker"))
        .expect("Architecture service terminal");
    let service_title = service
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .expect("Architecture service title");
    assert_eq!(
        service_title
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>(),
        "Worker"
    );
    assert_eq!(service_title.attribute("style"), Some("fill:#345678;"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 4);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 3);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn architecture_terminal_surface_plan_preserves_explicit_mermaid_owners() {
    let source = r#"architecture-beta
  service api[API]
  service worker[Worker]
  service db[Database]
  api:R --> L:worker
"#;
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "textColor": "#aabbcc",
            "archEdgeArrowColor": "#bbccdd",
            "archGroupBorderColor": "#ccddee"
        }
    })));
    let rendered = render_architecture_with_theme_and_engine(
        source,
        &architecture_terminal_surface_theme(),
        engine,
        "architecture-surface-owners",
    );
    let document = roxmltree::Document::parse(rendered.svg())
        .expect("valid source-owned Architecture surface SVG");

    let service_backgrounds = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && has_class(node, "node-bkg")
                && node.attribute("id").is_some_and(|id| id.contains("-node-"))
        })
        .collect::<Vec<_>>();
    assert!(
        service_backgrounds
            .iter()
            .all(|node| { node.attribute("style") == Some("fill:#123456;") })
    );
    assert!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .all(|text| text.attribute("style") == Some(""))
    );
    assert!(
        architecture_arrow_polygons(&document)
            .iter()
            .all(|arrow| arrow.attribute("style").is_none())
    );
    let css = stylesheet(rendered.svg());
    assert_eq!(
        stylesheet_property(&css, "#architecture-surface-owners", "fill"),
        Some("#aabbcc")
    );
    assert_eq!(
        stylesheet_property(&css, "#architecture-surface-owners .arrow", "fill"),
        Some("#bbccdd")
    );
    assert_eq!(
        stylesheet_property(&css, "#architecture-surface-owners .node-bkg", "stroke"),
        Some("#ccddee")
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 4);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 3);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn architecture_title_theme_remains_a_compatibility_residual_until_retirement_is_authorized() {
    let source = r#"architecture-beta
  service api[API]
"#;
    let theme = architecture_edge_rules_theme([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#abcdef").expect("valid Architecture title fill")),
    )
    .for_family(DiagramFamilyId::ARCHITECTURE)]);
    let error = match try_render_architecture_with_theme_and_engine(
        source,
        &theme,
        Engine::new(),
        "architecture-no-title-terminal",
    ) {
        Ok(_) => panic!("Architecture Title.fill must remain a compatibility residual"),
        Err(error) => error,
    };
    match error {
        merman_render::Error::LegacyFamilyThemeCompatibility {
            family_id,
            residual_count,
        } => {
            assert_eq!(family_id, DiagramFamilyId::ARCHITECTURE);
            assert_eq!(residual_count, 1);
        }
        other => panic!("expected Architecture legacy compatibility residual, got {other}"),
    }
}

#[test]
fn architecture_edge_stroke_respects_source_and_site_mermaid_owners() {
    let theme = architecture_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid Architecture Edge stroke"),
    );

    for (source_name, config) in [
        (
            "arch-edge",
            serde_json::json!({ "themeVariables": { "archEdgeColor": "#fedcba" } }),
        ),
        (
            "line-fallback",
            serde_json::json!({ "themeVariables": { "lineColor": "#fedcba" } }),
        ),
    ] {
        let source_config =
            serde_json::to_string(&config).expect("serialize Architecture source config");
        let source_owned = format!("%%{{init: {source_config}}}%%\n{TWO_ARCHITECTURE_EDGES}");
        let cases = [
            (
                "site",
                TWO_ARCHITECTURE_EDGES.to_string(),
                Engine::new().with_site_config(MermaidConfig::from_value(config.clone())),
            ),
            (
                "source",
                source_owned,
                Engine::new().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({ "secure": [] }),
                )),
            ),
        ];

        for (owner, source, engine) in cases {
            let diagram_id = format!("architecture-edge-{source_name}-{owner}");
            let rendered =
                render_architecture_with_theme_and_engine(&source, &theme, engine, &diagram_id);
            let document = roxmltree::Document::parse(rendered.svg())
                .expect("valid source-owned Architecture Edge SVG");
            let edges = architecture_edge_paths(&document);
            let css = stylesheet(rendered.svg());

            assert_eq!(edges.len(), 2, "{source_name}/{owner}");
            assert!(
                edges.iter().all(|edge| edge
                    .attribute("style")
                    .is_none_or(|style| !style.contains("#123456"))),
                "{source_name}/{owner}: typed Edge.stroke must not overwrite the Mermaid owner",
            );
            assert!(
                css.contains(&format!(
                    "#{diagram_id} .edge{{stroke-width:3;stroke:#fedcba;fill:none;}}"
                )),
                "{source_name}/{owner}: Mermaid's resolved archEdgeColor must remain terminal",
            );

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{source_name}/{owner}");
            assert_eq!(evidence.applied_count(), 0, "{source_name}/{owner}");
            assert_eq!(evidence.not_applicable_count(), 1, "{source_name}/{owner}");
            assert_eq!(evidence.theme_residual_count(), 0, "{source_name}/{owner}");
        }
    }
}

#[test]
fn architecture_edge_stroke_is_not_applicable_without_edge_occurrences() {
    let theme = architecture_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid Architecture Edge stroke"),
    );
    let rendered = render_architecture_with_theme_and_engine(
        ARCHITECTURE_WITHOUT_EDGES,
        &theme,
        Engine::new(),
        "architecture-edge-empty",
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid empty-edge Architecture SVG");

    assert!(architecture_edge_paths(&document).is_empty());
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn architecture_edge_stroke_rejects_ordinal_variant_clear_gradient_and_pattern_routes() {
    let stops = || {
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#123456").expect("valid Architecture gradient start"),
            )
            .expect("valid Architecture gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#abcdef").expect("valid Architecture gradient end"),
            )
            .expect("valid Architecture gradient stop"),
        ]
    };
    let linear = LinearGradient::new(90.0, stops()).expect("valid Architecture linear gradient");
    let radial = RadialGradient::new(
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        stops(),
    )
    .expect("valid Architecture radial gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#123456").expect("valid Architecture pattern color"),
    )
    .expect("valid Architecture pattern");
    let mut clear = ThemeStylePatch::default();
    clear.stroke.paint = Specified::Clear;
    let solid = || {
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#123456").expect("valid Architecture stroke"))
    };
    let cases = [
        ThemeRule::new(ThemeTarget::Edge, solid())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid Architecture Edge ordinal")),
        ThemeRule::new(ThemeTarget::Edge, solid()).with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::Edge, clear),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::LinearGradient(linear)),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::RadialGradient(radial)),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::Pattern(pattern)),
        ),
    ];

    for rule in cases {
        let theme = architecture_edge_rules_theme([rule.for_family(DiagramFamilyId::ARCHITECTURE)]);
        let error = match try_render_architecture_with_theme_and_engine(
            TWO_ARCHITECTURE_EDGES,
            &theme,
            Engine::new(),
            "architecture-edge-unsupported",
        ) {
            Ok(_) => panic!("unsupported Architecture Edge.stroke route must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::ARCHITECTURE, 1))
        );
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
        (left.2 - 160.0).abs() <= 1.0e-9,
        "custom architecture.padding should follow Cytoscape compound child, parent border, and final-bbox phases, got width {}",
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
        (max_width - 187.85890197753906).abs() < 0.001,
        "vertical edge label createText bbox should contribute to the root width, got {max_width}"
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
        pipeline.2 > 460.0 && pipeline.2 < 473.5,
        "long-title group width should use Cytoscape's Helvetica Canvas font stack: {}",
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
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
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
fn architecture_icon_text_clamp_uses_architecture_font_size() {
    let svg = render_architecture_fixture_with_options(
        "upstream_architecture_docs_service_icon_text.mmd",
        &SvgRenderOptions {
            diagram_id: Some("architecture-icontext".to_string()),
            ..Default::default()
        },
    );

    let clamp = icon_text_line_clamp(&svg, "architecture-icontext-service-with_icon_text");
    assert_eq!(
        clamp, 4,
        "iconText clamp should follow default architecture.fontSize=16 with iconSize=80"
    );
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
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions::headless_svg_defaults();
    let artifact = family::prepare(parsed.clone(), &layout_options, session)
        .expect("prepare Architecture artifact");
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
        "Architecture must not bypass the session with a family-local vendored measurer"
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
    let parity_artifact = family::prepare(parsed, &layout_options, parity_session)
        .expect("prepare parity Architecture artifact");
    let parity_rendered = parity_artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render parity Architecture artifact");
    let completion = parity_rendered.into_completion();
    assert_eq!(
        completion.report().family_id(),
        DiagramFamilyId::ARCHITECTURE
    );
    let (parity_svg, _) = completion.into_output_and_report();
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
                merman_core::runtime::RuntimePolicy::deterministic().with_fixed_seed(ambient_seed),
            )
            .begin_session()
            .expect("begin render session");
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse ok")
            .expect("diagram detected");
        let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
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
        let (svg, _) = completion.into_output_and_report();
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
