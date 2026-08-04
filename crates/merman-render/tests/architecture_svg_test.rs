#![cfg(feature = "layout-cytoscape")]

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family::{self, RenderFamilyKind};
use merman_render::model::ArchitectureDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::TextMetrics;
use merman_render::{LayoutOptions, RenderResourcePolicy};
use regex::Regex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
    prepare_architecture_text_with_engine_and_environment(
        engine,
        text,
        RenderEnvironment::deterministic(),
    )
}

fn prepare_architecture_text_with_engine_and_environment(
    engine: &Engine,
    text: &str,
    environment: RenderEnvironment,
) -> family::FamilyRenderArtifact {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions::headless_svg_defaults();
    let session = environment.begin_session().expect("begin render session");
    family::prepare(parsed, &layout_options, session).expect("layout ok")
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
        let artifact = prepare_architecture_text_with_engine_and_environment(
            &engine,
            &source,
            RenderEnvironment::deterministic()
                .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input()),
        );
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
        (left.2 - 162.0).abs() <= 1.0e-9,
        "custom architecture.padding should follow Cytoscape label, border, and final-bbox phases, got width {}",
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
        "compound label bottom should follow architecture.fontSize + 1px for custom font sizes, got group height {}",
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
    let (host_svg, family_kind, _, session) = rendered.into_parts();
    assert_eq!(family_kind, RenderFamilyKind::Architecture);

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
        operations.contains(&TextMeasurementOperation::TspanBBoxWidth),
        "Architecture root bounds must request the emitted outer tspan bbox width"
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
        session
            .text_measurement_report()
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
        session
            .text_measurement_report()
            .entries()
            .iter()
            .any(|entry| {
                entry.provenance().operation == TextMeasurementOperation::TspanBBoxWidth
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
    let (parity_svg, family_kind, _, _) = parity_rendered.into_parts();
    assert_eq!(family_kind, RenderFamilyKind::Architecture);
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
        let (svg, family_kind, _, _) = rendered.into_parts();
        assert_eq!(family_kind, RenderFamilyKind::Architecture);
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
