#![cfg(feature = "layout-cytoscape")]

type FamilyFixture = (
    merman_model::presentation::PresentationDiagram,
    merman_render::LayoutOptions,
    merman_render::environment::RenderSession,
);

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::ArchitectureDiagramLayout;
use merman_render::svg::{IconPack, IconRegistry, SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMetrics, TextStyle};
use merman_render::theme::{NativeTheme, ThemeCompiler};
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

#[test]
fn native_theme_v2_architecture_layers_service_cards_and_preserves_edge_color() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  themeVariables:\n    archEdgeColor: '#123456'\n---\narchitecture-beta\ngroup backend(cloud)[Backend]\nservice api(server)[API] in backend\nservice db[Database] in backend\napi:R --> L:db\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"studio",
                "roles":{"surface.node":"#DDEEFF","surface.container":"#E0E8F0",
                         "stroke.border":"#405060","text.title":"#243648"},
                "typography":{"sans":["Architecture Text","sans-serif"],"base_size_px":20},
                "form":{"radius":7,"container_radius":11,"connector_width":3,
                        "edge_style":"orthogonal","marker_style":"open","elevation":"soft"}
            }"##,
            None,
        )
        .unwrap();
    let prepared = family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap(),
    )
    .unwrap();
    let rendered = prepared
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let nodes: Vec<_> = document.descendants().collect();
    let position = |class| {
        nodes
            .iter()
            .position(|node| node.attribute("class") == Some(class))
            .unwrap()
    };
    let group = position("architecture-group-surfaces");
    let depth = nodes
        .iter()
        .position(|node| node.attribute("data-merman-theme-depth") == Some("node"))
        .unwrap();
    let edges = position("architecture-edges");
    let card = position("architecture-native-service-card");
    assert!(group < depth && depth < edges && edges < card);
    assert!(nodes[group].children().any(|node| {
        node.attribute("rx") == Some("11")
            && node
                .attribute("style")
                .is_some_and(|style| style.contains("fill:#E0E8F0"))
    }));
    assert_eq!(nodes[card].attribute("rx"), Some("7"));
    assert_eq!(nodes[card].attribute("fill"), Some("#DDEEFF"));
    let edge = nodes[position("edge")];
    assert_eq!(edge.attribute("d").unwrap().matches('L').count(), 3);
    assert!(nodes[position("arrow arrow-open")].has_tag_name("polyline"));
    let css = nodes
        .iter()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains("font-family:\"Architecture Text\", sans-serif;font-size:20px;"));
    assert!(css.contains(".edge{stroke-width:3px;stroke:#123456;"));
}

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

fn prepare_architecture_with_environment(
    source: &str,
    engine: MermaidEngine,
    environment: RenderEnvironment,
) -> FamilyFixture {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Architecture")
        .expect("detect Architecture")
        .into_presentation();
    let session = environment
        .begin_session()
        .expect("begin Architecture session");
    (parsed, LayoutOptions::headless_svg_defaults(), session)
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
        let theme = ThemeCompiler::new().compile_json(
            format!(r##"{{"schema_version":2,"extends":"paper","roles":{{"surface.container":"{fill}"}}}}"##).as_bytes(),
            None,
        ).unwrap();
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap();
        let rendered = family::prepare(
            parsed.presentation(),
            &LayoutOptions::headless_svg_defaults(),
            session,
        )
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
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
                Some(format!("fill:{fill};stroke-dasharray:none;").as_str())
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
        let edges = group("architecture-edges");
        assert!(
            edges.descendants().any(|node| {
                node.has_tag_name("path") && node.attribute("class") == Some("edge")
            })
        );
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
    let font_family = "ArchitectureClamp";
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
        for owner in ["site", "source"] {
            let mut site = json!({
                "architecture": {"fontSize": 16, "numIter": 1, "randomize": false}
            });
            let font_config = json!({"themeVariables": {"fontSize": format!("{size}px"), "fontFamily": font_family}});
            let source = match owner {
                "site" => {
                    site["themeVariables"] = font_config["themeVariables"].clone();
                    body.to_owned()
                }
                "source" => format!("---\nconfig: {font_config}\n---\n{body}"),
                _ => unreachable!(),
            };
            let host = Arc::new(CountingArchitectureHost::default());
            let (parsed, options, session) = prepare_architecture_with_environment(
                &source,
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
                    .filter(|style| style.font_family.as_deref() == Some(font_family))
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
fn architecture_host_root_sizes_preserve_css() {
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
            let parsed = MermaidEngine::new()
                .with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                )
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let host = Arc::new(CountingArchitectureHost::default());
            let session = counting_architecture_environment(Arc::clone(&host))
                .begin_session()
                .unwrap();
            let rendered =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
            assert!(
                architecture_stylesheet(rendered.svg()).contains(&format!("font-size:{size};"))
            );
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
    for expected_size in [0.0, 0.5] {
        let prefix = format!(
            "---\nconfig: {{themeVariables: {{fontSize: {expected_size}, fontFamily: ArchitectureSmall}}}}\n---\n"
        );
        let source = format!(
            "{prefix}architecture-beta\ngroup core [Group<br/>Second]\nservice api [API<br/>Second] in core\nservice db\napi:R -[Edge<br/>Second]- L:db\n"
        );
        let host = Arc::new(CountingArchitectureHost::default());
        let (parsed, options, session) = prepare_architecture_with_environment(
            &source,
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
    }
}

#[test]
fn architecture_small_service_text_rows_use_the_emitted_em_step() {
    for (size, expected_delta) in [(0.0, 0.0), (0.5, 0.55)] {
        let render_height = |label: &str| {
            let source = format!(
                "---\nconfig: {{themeVariables: {{fontSize: {size}}}}}\n---\narchitecture-beta\nservice api [{label}]\n"
            );
            let (parsed, options, session) = prepare_architecture_with_environment(
                &source,
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
