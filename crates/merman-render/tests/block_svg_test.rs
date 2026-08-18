mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};
use regex::Regex;

fn render_block_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    render_block_svg_from_text_with_engine(&engine, text)
}

fn render_block_svg_from_text_with_engine(engine: &Engine, text: &str) -> String {
    try_render_block_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
    .expect("svg render ok")
}

fn try_render_block_svg_from_text_with_engine(
    engine: &Engine,
    text: &str,
) -> merman_render::Result<String> {
    try_render_block_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
}

fn try_render_block_svg_from_text_with_engine_and_policy(
    engine: &Engine,
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;

    Ok(artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?
        .svg()
        .to_owned())
}

fn translated_center(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let transform = node.attribute("transform").expect("node transform");
    let captures = Regex::new(
        r"^translate\(\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*\)$",
    )
    .expect("valid transform regex")
    .captures(transform)
    .expect("translate(x, y) transform");
    (
        captures[1].parse().expect("numeric translate x"),
        captures[2].parse().expect("numeric translate y"),
    )
}

fn path_start(path: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let d = path.attribute("d").expect("path data");
    let captures =
        Regex::new(r"^M\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))")
            .expect("valid path regex")
            .captures(d)
            .expect("path starts with an absolute move");
    (
        captures[1].parse().expect("numeric path x"),
        captures[2].parse().expect("numeric path y"),
    )
}

fn deep_block_chain(depth: usize) -> String {
    let mut input = String::from("block\n");
    for level in 0..depth {
        input.push_str(&format!("block:n{level}[\"n{level}\"]\n"));
    }
    input.push_str("leaf[\"leaf\"]\n");
    for _ in 0..depth {
        input.push_str("end\n");
    }
    input
}

fn block_node_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile Block node stroke theme")
}

fn render_block_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Block")
        .expect("detect themed Block");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Block session");

    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Block")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("block-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Block")
}

fn terminal_shell_styles(svg: &str, node_id: &str) -> Vec<(String, String)> {
    let document = roxmltree::Document::parse(svg).expect("valid Block SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Block node {node_id}: {svg}"));

    node.descendants()
        .filter_map(|shell| {
            if !shell.is_element() {
                return None;
            }
            let tag = shell.tag_name().name();
            matches!(tag, "rect" | "circle" | "path" | "polygon")
                .then(|| shell.attribute("style").map(|style| (tag, style)))
                .flatten()
        })
        .map(|(tag, style)| (tag.to_string(), style.to_string()))
        .collect()
}

fn terminal_stroke(style: &str) -> Option<&str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case("stroke"))
        .map(|(_, value)| value.trim())
        .next_back()
}

#[test]
fn block_typed_node_stroke_reaches_every_terminal_shape() {
    let source = r#"block-beta
  columns 5
  rect["Rect"] circle(("Circle")) double((("Double"))) cylinder[("Cylinder")] polygon{{"Polygon"}}
"#;
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Block node stroke"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (stroke, expected_stroke) in cases {
        let theme = block_node_stroke_theme(stroke);
        let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());
        let expected_shells = [
            ("block-theme-rect", ["rect"].as_slice()),
            ("block-theme-circle", ["circle"].as_slice()),
            ("block-theme-double", ["circle", "circle"].as_slice()),
            ("block-theme-cylinder", ["path"].as_slice()),
            ("block-theme-polygon", ["polygon"].as_slice()),
        ];

        for (node_id, expected_tags) in expected_shells {
            let shells = terminal_shell_styles(rendered.svg(), node_id);
            assert_eq!(
                shells
                    .iter()
                    .map(|(tag, _)| tag.as_str())
                    .collect::<Vec<_>>(),
                expected_tags,
                "unexpected terminal shells for {node_id}: {}",
                rendered.svg()
            );
            assert!(
                shells
                    .iter()
                    .all(|(_, style)| terminal_stroke(style) == Some(expected_stroke)),
                "typed stroke must reach every terminal shell for {node_id}: {shells:?}"
            );
        }

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn block_inline_and_class_strokes_outrank_typed_node_stroke_per_node() {
    let source = r#"block-beta
  columns 3
  bare["Bare"] assigned(("Assigned")) inline{{"Inline"}}
  classDef accent stroke:#00bb00
  class assigned accent
  style inline stroke:#0000bb
"#;
    let theme =
        block_node_stroke_theme(CanvasPaint::solid("#123456").expect("valid Block node stroke"));
    let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());

    let bare = terminal_shell_styles(rendered.svg(), "block-theme-bare");
    assert_eq!(terminal_stroke(&bare[0].1), Some("#123456"), "{bare:?}");
    let inline = terminal_shell_styles(rendered.svg(), "block-theme-inline");
    assert_eq!(terminal_stroke(&inline[0].1), Some("#0000bb"), "{inline:?}");
    let assigned = terminal_shell_styles(rendered.svg(), "block-theme-assigned");
    assert!(
        assigned
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#123456")),
        "assigned class must suppress typed inline stroke: {assigned:?}"
    );
    assert!(
        rendered
            .svg()
            .contains(r#"#block-theme .accent&gt;*{stroke:#00bb00!important;}"#),
        "assigned class stroke must retain source ownership: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_default_class_stroke_makes_typed_node_stroke_not_applicable() {
    let source = r#"block
  A["Alpha"]
  classDef default stroke:#bb0000
"#;
    let theme = block_node_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid suppressed Block node stroke"),
    );
    let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#123456")),
        "default class must suppress typed inline stroke: {shells:?}"
    );
    assert!(
        rendered
            .svg()
            .contains(r#"#block-theme .default&gt;*{stroke:#bb0000!important;}"#),
        "default class stroke must retain source ownership: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_explicit_node_border_outranks_typed_node_stroke() {
    let theme = block_node_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid config-suppressed Block node stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "nodeBorder": "#445566" }
    })));
    let rendered = render_block_with_theme_and_engine("block\n  A[\"Alpha\"]\n", &theme, engine);
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#123456")),
        "explicit nodeBorder must suppress typed inline stroke: {shells:?}"
    );
    assert!(
        rendered.svg().contains("stroke:#445566;stroke-width:1px"),
        "explicit nodeBorder must remain the Block base stroke: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_svg_scopes_text_and_edge_colors_for_html_labels() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        !svg.contains("<style></style>"),
        "expected block SVG to emit scoped CSS instead of an empty style element"
    );
    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#333;color:#333;}"#),
        "expected block HTML/SVG labels to avoid inheriting host page text color"
    );
    assert!(
        svg.contains(r#"#merman .flowchart-link{stroke:#333333;fill:none;}"#),
        "expected block edges to carry their scoped stroke color"
    );
}

#[test]
fn block_public_svg_render_handles_deep_chain() {
    const DEPTH: usize = 1200;
    let svg = try_render_block_svg_from_text_with_engine_and_policy(
        &Engine::new(),
        &deep_block_chain(DEPTH),
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("deep Block SVG render");

    assert!(
        svg.contains(r#"id="merman-leaf""#),
        "expected deep Block leaf to render without stack-dependent traversal"
    );
}

#[test]
fn block_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"block
  A["Alpha"] --> B["Beta"]
  classDef branded fill:#696,stroke:#333
  class A branded
"#;
    let engine = Engine::new();
    let baseline = try_render_block_svg_from_text_with_engine_and_policy(
        &engine,
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Block baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Block fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Block SVG byte ceiling");
    let exact =
        try_render_block_svg_from_text_with_engine_and_policy(&engine, source, exact_policy)
            .expect("the exact Block family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Block SVG byte ceiling");
    let error =
        try_render_block_svg_from_text_with_engine_and_policy(&engine, source, below_policy)
            .expect_err("one byte below the Block family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Block MaxSvgBytes rejection, got {error}");
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
fn block_svg_honors_visible_edge_stroke_width_theme() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"strokeWidth": 4, "lineColor": "#112233"}}}%%
block
  A --> B
"##,
    );

    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:4px;}"#),
        "expected shared Mermaid edge thickness CSS to reach visible Block edges: {svg}"
    );
    assert!(
        svg.contains(r#"class="edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1""#),
        "expected Block edge path to carry the themed edge-thickness-normal class: {svg}"
    );
}

#[test]
fn block_svg_uses_mermaid_11_15_dom_ids_and_html_label_shape() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        svg.contains(r#"id="merman-A""#),
        "expected Block node DOM id to be diagram-prefixed: {svg}"
    );
    assert!(
        svg.contains(r#"id="merman-1-A-B""#),
        "expected Block edge DOM id to be diagram-prefixed: {svg}"
    );
    assert!(
        svg.contains(r#"style="display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="nodeLabel"><p>Alpha</p></span>"#),
        "expected Block node label to use Mermaid 11.15 XHTML paragraph shape: {svg}"
    );
}

#[test]
fn block_svg_keeps_blank_placeholder_label_paragraph() {
    let svg = render_block_svg_from_text(
        r#"block
  blockArrowId6<["   "]>(down)
"#,
    );

    assert!(
        svg.contains(r#"<span class="nodeLabel"><p>   </p></span>"#),
        "expected blank Block placeholder labels to keep Mermaid's paragraph child: {svg}"
    );
}

#[test]
fn block_svg_honors_configured_node_text_color() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "nodeTextColor": "#123456"
        }
    })));
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r#"block
  A["Alpha"]
"#,
    );

    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#123456;color:#123456;}"#),
        "expected nodeTextColor theme variable to drive block label color"
    );
}

#[test]
fn block_svg_fades_cluster_theme_colors() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"clusterBkg": "rebeccapurple", "clusterBorder": "hsl(80, 100%, 96.2745098039%)"}}}%%
block
  block
    A["Alpha"]
  end
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node .cluster{fill:rgba(102, 51, 153, 0.5);stroke:rgba(248.6666666666, 255, 235.9999999999, 0.2);stroke-width:1px;}"#
        ),
        "expected block composite cluster CSS to follow Mermaid 11.15 fade() colors"
    );
}

#[test]
fn block_svg_rejects_unsupported_cluster_theme_color() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "clusterBkg": "not-a-css-color"
        }
    })));
    let error = try_render_block_svg_from_text_with_engine(
        &engine,
        r#"block
  block
    A["Alpha"]
  end
"#,
    )
    .expect_err("unsupported khroma color must fail the render operation");

    assert!(error.to_string().contains("not-a-css-color"));
}

#[test]
fn block_svg_normalizes_khroma_colors_and_preserves_css_tokens() {
    let svg = render_block_svg_from_text(
        r#"block
  A["HSL"]
  B["Alpha"]
  C["Variable"]
  style A color:hsl(80 100% 96%)
  style B color:#8090a080
  style C color:var(--MyColor)
"#,
    );

    assert!(svg.contains("color: rgb(248, 255, 235); display: table-cell;"));
    assert!(svg.contains("color: rgba(128, 144, 160, 0.502); display: table-cell;"));
    assert!(svg.contains("color: var(--MyColor); display: table-cell;"));
}

#[test]
fn block_svg_applies_class_definitions_to_assigned_nodes() {
    let svg = render_block_svg_from_text(
        r#"block
  Frontend Backend Database[("Database")]

  classDef front fill:#696,stroke:#333;
  classDef back fill:#969,stroke:#333;
  class Frontend front
  class Backend,Database back
"#,
    );

    assert!(
        svg.contains(r#"class="node default front flowchart-label""#),
        "expected the Frontend node to retain its assigned class: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .front&gt;*{fill:#696!important;stroke:#333!important;}"#),
        "expected the front class definition to style Block shapes: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .back&gt;*{fill:#969!important;stroke:#333!important;}"#),
        "expected the back class definition to style Block shapes: {svg}"
    );
}

#[test]
fn block_svg_xml_escapes_class_definition_values() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"]
  classDef branded font-family:"A&B",fill:#696
  class A branded
"#,
    );

    roxmltree::Document::parse(&svg).expect("classDef CSS must remain well-formed XML");
    assert!(
        svg.contains(r#"font-family:&quot;A&amp;B&quot;!important;"#),
        "expected classDef values to be XML escaped inside the style element: {svg}"
    );
}

#[test]
fn block_circle_edge_starts_on_the_rendered_circle_boundary() {
    let svg = render_block_svg_from_text(
        r##"block-beta
  columns 3
  user(("User")):3
  space:3
  ui["Web UI"] api["API Server"] db[("Database")]

  user --> ui
  ui --> api
  api --> db

  style user fill:#ffe0b2,stroke:#fb8c00
  style db fill:#bbdefb,stroke:#1e88e5
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let user = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-user"))
        .expect("rendered user node");
    let circle = user
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .expect("rendered user circle");
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "flowchart-link")
                })
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("user-ui"))
        })
        .expect("user to ui edge");

    let (center_x, center_y) = translated_center(user);
    let (edge_x, edge_y) = path_start(edge);
    let radius: f64 = circle
        .attribute("r")
        .expect("circle radius")
        .parse()
        .expect("numeric circle radius");
    let endpoint_radius = ((edge_x - center_x).powi(2) + (edge_y - center_y).powi(2)).sqrt();

    assert!(
        (endpoint_radius - radius).abs() <= 1e-3,
        "edge must start on the rendered circle: center=({center_x},{center_y}), endpoint=({edge_x},{edge_y}), endpoint_radius={endpoint_radius}, circle_radius={radius}, svg={svg}"
    );
}

#[test]
fn block_short_stadium_edge_starts_on_the_svg_clamped_boundary() {
    let svg = render_block_svg_from_text(
        r#"block
  A(["A"]) --> B["B"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let stadium = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-A"))
        .expect("rendered stadium node");
    let rect = stadium
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("width").is_some())
        .expect("rendered stadium outline");
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path") && node.attribute("id").is_some_and(|id| id.contains("A-B"))
        })
        .expect("stadium edge");

    let (center_x, center_y) = translated_center(stadium);
    let (edge_x, edge_y) = path_start(edge);
    let width: f64 = rect
        .attribute("width")
        .expect("stadium width")
        .parse()
        .expect("numeric stadium width");
    let height: f64 = rect
        .attribute("height")
        .expect("stadium height")
        .parse()
        .expect("numeric stadium height");

    assert!(width < height, "fixture must exercise SVG radius clamping");
    assert!((edge_x - (center_x + width / 2.0)).abs() <= 1e-3);
    assert!((edge_y - center_y).abs() <= 1e-3);
}
