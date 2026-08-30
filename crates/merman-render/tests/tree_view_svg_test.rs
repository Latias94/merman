mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{
    Engine, MAX_DIAGRAM_NESTING_DEPTH, MermaidConfig, ParseOptions, ParsedDiagramRender,
};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, InsetsPx,
    OrdinalSelector, Specified, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStrokePatch, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, RenderSession, TextMeasurementOperation,
    TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::TreeViewDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{IconPack, IconRegistry, SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_render::{DiagramFamilyId, LayoutOptions};
use std::sync::Arc;
use std::sync::Mutex;

fn tree_view_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Tree View edge theme")
}

fn tree_view_edge_width_style(stroke_width: Specified<f32>) -> ThemeStylePatch {
    ThemeStylePatch {
        stroke: ThemeStrokePatch {
            width: stroke_width,
            ..ThemeStrokePatch::default()
        },
        ..ThemeStylePatch::default()
    }
}

fn tree_view_edge_width_theme(stroke_width: Specified<f32>) -> DiagramTheme {
    tree_view_rules_theme([ThemeRule::new(
        ThemeTarget::Edge,
        tree_view_edge_width_style(stroke_width),
    )])
}

fn tree_view_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::TREE_VIEW, typography),
        ))
        .expect("compile Tree View typography theme")
}

fn tree_view_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Tree View SVG XML");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect()
}

fn tree_view_css_rule<'a>(stylesheet: &'a str, selector: &str) -> &'a str {
    let marker = format!("{selector} {{");
    let start = stylesheet
        .find(&marker)
        .unwrap_or_else(|| panic!("missing Tree View CSS selector `{selector}`: {stylesheet}"))
        + marker.len();
    let end = stylesheet[start..].find('}').unwrap_or_else(|| {
        panic!("unterminated Tree View CSS selector `{selector}`: {stylesheet}")
    });
    &stylesheet[start..start + end]
}

fn try_render_tree_view_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<(TreeViewDiagramLayout, String)> {
    try_render_tree_view_svg_with_theme_requirement(
        source,
        theme,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn try_render_tree_view_svg_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<(TreeViewDiagramLayout, String)> {
    try_render_tree_view_svg_with_theme_requirement_and_engine(
        source,
        theme,
        Engine::new(),
        portability,
    )
}

fn try_render_tree_view_svg_with_theme_requirement_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<(TreeViewDiagramLayout, String)> {
    let (layout, rendered) = try_render_tree_view_with_theme_and_environment(
        source,
        theme,
        engine,
        portability,
        &RenderEnvironment::deterministic(),
        "tree-view-theme",
    )?;
    Ok((layout, rendered.svg().to_owned()))
}

fn try_render_tree_view_with_theme_and_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
    environment: &RenderEnvironment,
    diagram_id: &str,
) -> merman_render::Result<(TreeViewDiagramLayout, family::RenderedFamilySvg)> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Tree View")
        .expect("detect themed Tree View");
    let session = environment
        .clone()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Tree View session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let projection = artifact.layout_json()?;
    let layout = serde_json::from_value(projection["layout"]["TreeViewDiagram"].clone())
        .expect("Tree View layout projection");
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok((layout, rendered))
}

fn render_tree_view_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> (TreeViewDiagramLayout, String) {
    try_render_tree_view_svg_with_theme(source, theme).expect("render themed Tree View SVG")
}

#[derive(Default)]
struct TreeViewBBoxHost {
    operations: Mutex<Vec<TextMeasurementOperation>>,
}

impl HostTextMeasurer for TreeViewBBoxHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.operations.lock().unwrap().push(request.operation);
        Ok(match request.operation {
            TextMeasurementOperation::RawBBoxWidth => {
                Some(HostTextMeasurement::Length(request.text.len() as f64 * 8.0))
            }
            TextMeasurementOperation::RawBBoxHeight => Some(HostTextMeasurement::Length(31.0)),
            _ => None,
        })
    }
}

#[derive(Debug)]
struct TreeViewTypedFontMeasurer {
    expected_font_family: String,
    measured_text: Mutex<Vec<String>>,
}

impl TextMeasurer for TreeViewTypedFontMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        assert_eq!(
            style.font_family.as_deref(),
            Some(self.expected_font_family.as_str()),
            "Tree View layout measurement must use the resolved typed font stack"
        );
        self.measured_text.lock().unwrap().push(text.to_string());
        TextMetrics {
            width: text.chars().count() as f64 * 9.0,
            height: 19.0,
            line_count: 1,
        }
    }
}

fn render_tree_view_svg_with_options(input: &str, options: SvgRenderOptions) -> String {
    render_tree_view_svg_with_environment(input, options, &RenderEnvironment::deterministic())
}

fn render_tree_view_svg_with_environment(
    input: &str,
    options: SvgRenderOptions,
    environment: &RenderEnvironment,
) -> String {
    render_tree_view_svg_with_engine_and_environment(input, options, &Engine::new(), environment)
}

fn render_tree_view_svg_with_engine_and_environment(
    input: &str,
    options: SvgRenderOptions,
    engine: &Engine,
    environment: &RenderEnvironment,
) -> String {
    let session = environment.begin_session().unwrap();
    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .expect("TreeView diagram");
    render_parsed_tree_view_svg(parsed, options, session)
}

fn render_parsed_tree_view_svg(
    parsed: ParsedDiagramRender,
    options: SvgRenderOptions,
    session: RenderSession,
) -> String {
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    artifact
        .render_svg(&options, &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn try_render_tree_view_svg_with_resource_policy(
    input: &str,
    diagram_id: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin TreeView resource-bound session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse TreeView resource-bound fixture")
        .expect("detect TreeView resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn tree_view_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let input = r#"treeView-beta
Root/ :::highlight icon(folder) ## bounded root
    Child.txt icon(file) ## bounded child
"#;
    let diagram_id = "tree-view-bounded";
    let baseline = try_render_tree_view_svg_with_resource_policy(
        input,
        diagram_id,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded TreeView baseline");
    let exact_bytes = baseline.len();
    assert!(
        exact_bytes > 1,
        "TreeView fixture must emit a non-empty SVG"
    );

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact TreeView SVG byte ceiling");
    let exact = try_render_tree_view_svg_with_resource_policy(input, diagram_id, exact_policy)
        .expect("the exact TreeView family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact TreeView SVG byte ceiling");
    let error = try_render_tree_view_svg_with_resource_policy(input, diagram_id, below_policy)
        .expect_err("one byte below the TreeView family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected TreeView MaxSvgBytes rejection, got {error}");
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

fn layout_tree_view(input: &str, environment: &RenderEnvironment) -> TreeViewDiagramLayout {
    let session = environment.begin_session().unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .expect("TreeView diagram");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    serde_json::from_value(projection["layout"]["TreeViewDiagram"].clone()).unwrap()
}

#[test]
fn tree_view_typed_render_model_outputs_svg() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"---
config:
  treeView:
    rowIndent: 80
    lineThickness: 3
  themeVariables:
    treeView:
      labelFontSize: '20px'
      labelColor: '#FF0000'
      lineColor: '#00FF00'
---
treeView-beta
    "packages"
        "mermaid"
            "src"
        "parser"
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "treeView");

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(svg.contains(r#"aria-roledescription="treeView""#));
    assert!(svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"style="max-width: "#));
    assert!(svg.contains(r#"viewBox="-1.5 0 "#));
    assert!(svg.contains(r#"<g/><g class="tree-view">"#));
    assert!(svg.contains(r#"<g class="tree-view">"#));
    assert!(svg.contains(r#"<g><text dominant-baseline="middle""#));
    assert!(svg.contains(r#"class="treeView-node-label""#));
    assert!(svg.contains(r#"class="treeView-node-line""#));
    assert!(svg.contains(r#"font-size: 20px"#));
    assert!(svg.contains(r#"fill: #FF0000"#));
    assert!(svg.contains(r#"stroke: #00FF00"#));
}

#[test]
fn tree_view_typed_font_stack_reaches_layout_css_and_terminal_evidence() {
    let font_stack =
        FontStack::new(["Tree View Typed", "monospace"]).expect("valid Tree View font stack");
    let expected_font = font_stack.as_css();
    let theme = tree_view_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let measurer = Arc::new(TreeViewTypedFontMeasurer {
        expected_font_family: expected_font.clone(),
        measured_text: Mutex::new(Vec::new()),
    });
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.tree-view-typed-font").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, measurer.clone())),
    );
    let (layout, rendered) = try_render_tree_view_with_theme_and_environment(
        "treeView-beta\nRoot/\n    Child.txt ## typed description\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
        &environment,
        "tree-view-typed-font",
    )
    .expect("render directly themed Tree View through the public family entry");

    assert!(layout.nodes.iter().all(|node| node.label_height == 19.0));
    let measured_text = measurer.measured_text.lock().unwrap();
    assert!(
        measured_text.iter().any(|text| text == "Root")
            && measured_text.iter().any(|text| text == "Child.txt"),
        "Tree View labels must reach the recording measurer"
    );
    assert!(
        measured_text.iter().any(|text| text == "typed description"),
        "Tree View descriptions must use the same typed layout font"
    );
    drop(measured_text);

    let stylesheet = tree_view_stylesheet(rendered.svg());
    for selector in [".treeView-node-label", ".treeView-node-description"] {
        assert!(
            tree_view_css_rule(&stylesheet, selector)
                .contains(&format!("font-family: {expected_font};")),
            "missing resolved Tree View font rule for {selector}: {stylesheet}"
        );
    }

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn tree_view_node_label_fill_overrides_the_generic_text_fallback() {
    let source = "treeView-beta\nRoot/\n    Child\n";
    let generic_color = CanvasPaint::solid("#123456").expect("valid generic Text color");
    let generic_theme = tree_view_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(generic_color.clone()),
    )]);
    let (_, generic_rendered) = try_render_tree_view_with_theme_and_environment(
        source,
        &generic_theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
        &RenderEnvironment::deterministic(),
        "tree-view-generic-text",
    )
    .expect("render Tree View with the generic Text fallback");
    let generic_stylesheet = tree_view_stylesheet(generic_rendered.svg());
    assert!(
        tree_view_css_rule(&generic_stylesheet, ".treeView-node-label").contains("fill: #123456;"),
        "generic Text.fill must reach the Tree View label terminal: {generic_stylesheet}"
    );
    let generic_evidence =
        merman_render::__private::family_evidence(generic_rendered.into_completion().report());
    assert_eq!(generic_evidence.required_count(), 1);
    assert_eq!(generic_evidence.accounted_count(), 1);
    assert_eq!(generic_evidence.applied_count(), 1);
    assert_eq!(generic_evidence.not_applicable_count(), 0);
    assert_eq!(generic_evidence.theme_residual_count(), 0);
    assert_eq!(generic_evidence.compatibility_residual_count(), 0);

    let label_color = CanvasPaint::solid("#654321").expect("valid NodeLabel color");
    let cascade_theme = tree_view_rules_theme([
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(generic_color),
        ),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(label_color),
        ),
    ]);
    let (_, rendered) = try_render_tree_view_with_theme_and_environment(
        source,
        &cascade_theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
        &RenderEnvironment::deterministic(),
        "tree-view-node-label-override",
    )
    .expect("render Tree View with the specific NodeLabel winner");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Tree View SVG");
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node.text() == Some("Child")
            && node.attribute("class").is_some_and(|class| {
                class
                    .split_whitespace()
                    .any(|part| part == "treeView-node-label")
            })
    }));
    drop(document);
    let stylesheet = tree_view_stylesheet(rendered.svg());
    let label_rule = tree_view_css_rule(&stylesheet, ".treeView-node-label");
    assert!(label_rule.contains("fill: #654321;"), "{stylesheet}");
    assert!(!label_rule.contains("#123456"), "{stylesheet}");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.mermaid_compatibility_residual_count(), 0);
}

#[test]
fn tree_view_static_edge_width_reaches_layout_bounds_endpoints_svg_and_evidence() {
    let source = "treeView-beta\nRoot/\n    Child\n";
    let baseline_layout = layout_tree_view(source, &RenderEnvironment::deterministic());
    let baseline_svg = render_tree_view_svg_with_options(
        source,
        SvgRenderOptions {
            diagram_id: Some("tree-view-theme".to_string()),
            ..SvgRenderOptions::default()
        },
    );
    let empty_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty typed theme");
    let (_, empty_theme_svg) = render_tree_view_svg_with_theme(source, &empty_theme);
    assert_eq!(empty_theme_svg.as_bytes(), baseline_svg.as_bytes());

    let (layout, svg) =
        render_tree_view_svg_with_theme(source, &tree_view_edge_width_theme(Specified::Value(6.0)));
    assert_eq!(layout.line_thickness, 6.0);
    assert_eq!(
        layout.bounds.as_ref().map(|bounds| bounds.min_x),
        Some(-3.0)
    );
    assert!(layout.lines.iter().all(|line| line.stroke_width == 6.0));

    let baseline_vertical = baseline_layout
        .lines
        .iter()
        .find(|line| line.kind == "vertical")
        .expect("baseline Tree View vertical line");
    let themed_vertical = layout
        .lines
        .iter()
        .find(|line| line.kind == "vertical")
        .expect("themed Tree View vertical line");
    assert_eq!(themed_vertical.y2 - baseline_vertical.y2, 2.5);

    let document = roxmltree::Document::parse(&svg).expect("valid themed Tree View SVG XML");
    let root_view_box = document
        .root_element()
        .attribute("viewBox")
        .expect("Tree View root viewBox");
    assert!(root_view_box.starts_with("-3 0 "), "{root_view_box}");
    let terminal_lines = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("treeView-node-line"))
        .collect::<Vec<_>>();
    assert_eq!(terminal_lines.len(), layout.lines.len());
    assert!(
        terminal_lines
            .iter()
            .all(|line| line.attribute("stroke-width") == Some("6"))
    );
}

#[test]
fn tree_view_edge_width_preserves_the_authored_round_trip_token() {
    let source = "treeView-beta\nRoot/\n    Child\n";
    let authored_width = 5.999_999_5_f32;
    let (layout, svg) = render_tree_view_svg_with_theme(
        source,
        &tree_view_edge_width_theme(Specified::Value(authored_width)),
    );

    assert_eq!(layout.line_thickness, f64::from(authored_width));
    let document = roxmltree::Document::parse(&svg).expect("valid themed Tree View SVG XML");
    assert!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("treeView-node-line"))
            .all(|line| line.attribute("stroke-width") == Some("5.9999995"))
    );
}

#[test]
fn tree_view_static_width_falls_back_when_an_ordinal_rule_wins_any_line() {
    let source = "treeView-beta\nRoot/\n    Child\n";
    let theme = tree_view_rules_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            tree_view_edge_width_style(Specified::Value(6.0)),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            tree_view_edge_width_style(Specified::Value(9.0)),
        )
        .with_ordinal(OrdinalSelector::exact(1).expect("valid Tree View edge ordinal")),
    ]);
    let (layout, svg) = try_render_tree_view_svg_with_theme_requirement(
        source,
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render Tree View with an unsupported ordinal width");

    assert_eq!(layout.line_thickness, 1.0);
    assert!(layout.lines.iter().all(|line| line.stroke_width == 1.0));
    let document = roxmltree::Document::parse(&svg).expect("valid fallback Tree View SVG XML");
    assert!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("treeView-node-line"))
            .all(|line| line.attribute("stroke-width") == Some("1"))
    );
}

#[test]
fn tree_view_large_edge_width_expands_paint_bounds_without_relaying_out_nodes() {
    let source = "treeView-beta\nRoot/\n    Child\n";
    let baseline = layout_tree_view(source, &RenderEnvironment::deterministic());
    let (layout, svg) = render_tree_view_svg_with_theme(
        source,
        &tree_view_edge_width_theme(Specified::Value(100.0)),
    );

    assert_eq!(layout.nodes.len(), baseline.nodes.len());
    for (themed, baseline) in layout.nodes.iter().zip(&baseline.nodes) {
        assert_eq!((themed.x, themed.y), (baseline.x, baseline.y));
        assert_eq!(
            (themed.label_x, themed.label_y),
            (baseline.label_x, baseline.label_y)
        );
    }

    let bounds = layout.bounds.as_ref().expect("Tree View paint bounds");
    let stroke_outset = layout.line_thickness / 2.0;
    for (line_index, line) in layout.lines.iter().enumerate() {
        let expected_min_x = if line_index == 0 && line.kind == "horizontal" {
            line.x2 - stroke_outset
        } else {
            line.x1.min(line.x2) - stroke_outset
        };
        assert!(bounds.min_x <= expected_min_x);
        assert!(bounds.min_y <= line.y1.min(line.y2) - stroke_outset);
        assert!(bounds.max_x >= line.x1.max(line.x2) + stroke_outset);
        assert!(bounds.max_y >= line.y1.max(line.y2) + stroke_outset);
    }
    assert!(bounds.min_y < 0.0);
    assert!(bounds.max_y > layout.total_height);

    let document = roxmltree::Document::parse(&svg).expect("valid wide-edge Tree View SVG XML");
    let view_box = document
        .root_element()
        .attribute("viewBox")
        .expect("Tree View root viewBox")
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    assert!((view_box[0] - bounds.min_x).abs() < 1e-6);
    assert!((view_box[1] - bounds.min_y).abs() < 1e-6);
    assert!((view_box[0] + view_box[2] - bounds.max_x).abs() < 1e-6);
    assert!((view_box[1] + view_box[3] - bounds.max_y).abs() < 1e-6);
}

#[test]
fn tree_view_edge_width_clear_restores_the_mermaid_baseline() {
    let source = "treeView-beta\nRoot/\n    Child\n";
    let (layout, svg) =
        render_tree_view_svg_with_theme(source, &tree_view_edge_width_theme(Specified::Clear));

    assert_eq!(layout.line_thickness, 1.0);
    let document = roxmltree::Document::parse(&svg).expect("valid cleared Tree View SVG XML");
    assert!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("treeView-node-line"))
            .all(|line| line.attribute("stroke-width") == Some("1"))
    );
}

#[test]
fn tree_view_explicit_line_thickness_outranks_typed_edge_width() {
    let source = r#"---
config:
  treeView:
    lineThickness: 7
---
treeView-beta
Root/
    Child
"#;
    let (layout, svg) =
        render_tree_view_svg_with_theme(source, &tree_view_edge_width_theme(Specified::Value(6.0)));

    assert_eq!(layout.line_thickness, 7.0);
    let document = roxmltree::Document::parse(&svg).expect("valid source-owned Tree View SVG XML");
    assert!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("treeView-node-line"))
            .all(|line| line.attribute("stroke-width") == Some("7"))
    );
}

#[test]
fn tree_view_legacy_marker_winner_outranks_typed_edge_icon_fallback() {
    let edge = CanvasPaint::solid("#00ff00").expect("valid Tree View edge color");
    let marker = CanvasPaint::solid("#0000ff").expect("valid Tree View marker color");
    let theme = tree_view_rules_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(edge),
        ),
        ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_stroke(marker),
        ),
    ]);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "securityLevel": "loose"
    })));
    let (_, svg) = try_render_tree_view_svg_with_theme_requirement_and_engine(
        "treeView-beta\nRoot icon(folder)\n",
        &theme,
        engine,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("render mixed Tree View edge and marker paint");
    let document = roxmltree::Document::parse(&svg).expect("valid mixed Tree View SVG XML");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Tree View stylesheet");

    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("use") && node.attribute("class") == Some("treeView-node-icon")
        }),
        "the fixture must exercise a real icon terminal"
    );
    assert!(
        stylesheet.contains(".treeView-node-line { stroke: #00ff00; }"),
        "{stylesheet}"
    );
    assert!(
        stylesheet.contains(".treeView-node-icon { color: #0000ff; }"),
        "{stylesheet}"
    );
}

#[test]
fn tree_view_edge_width_rejects_qualified_ordinal_and_mixed_rules() {
    let source = "treeView-beta\nRoot/\n    Child\n";
    let width_style = tree_view_edge_width_style(Specified::Value(6.0));
    for rule in [
        ThemeRule::new(ThemeTarget::Edge, width_style.clone()).with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::Edge, width_style.clone())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid Tree View edge ordinal")),
        ThemeRule::new(
            ThemeTarget::Edge,
            width_style.clone().with_padding(InsetsPx::all(4.0)),
        ),
    ] {
        let error = try_render_tree_view_svg_with_theme(source, &tree_view_rules_theme([rule]))
            .expect_err("unsupported Tree View edge routes must fail closed");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::TREE_VIEW, 1))
        );
    }

    let theme = tree_view_rules_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            tree_view_edge_width_style(Specified::Value(6.0)),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            tree_view_edge_width_style(Specified::Value(99.0)),
        )
        .with_variant(ThemeVariant::Warning)
        .with_ordinal(
            OrdinalSelector::exact(999).expect("valid out-of-range Tree View edge ordinal"),
        ),
    ]);
    let (_, svg) = try_render_tree_view_svg_with_theme(source, &theme)
        .expect("an out-of-range Tree View selector must be not applicable");
    let document = roxmltree::Document::parse(&svg).expect("valid themed Tree View SVG XML");
    assert!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("treeView-node-line"))
            .all(|line| line.attribute("stroke-width") == Some("6"))
    );
}

#[test]
fn tree_view_typed_render_model_outputs_accessibility_nodes() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"treeView-beta
title TreeView Diagram Title
accTitle: Accessible TreeView Title
accDescr: Accessible TreeView Description
"Root"
    "Child"
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "treeView");

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-a11y-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(svg.contains(
        r#"aria-describedby="chart-desc-tree-view-a11y-test" aria-labelledby="chart-title-tree-view-a11y-test""#
    ));
    assert!(svg.contains(
        r#"<title id="chart-title-tree-view-a11y-test">Accessible TreeView Title</title><desc id="chart-desc-tree-view-a11y-test">Accessible TreeView Description</desc><style>"#
    ));
}

#[test]
fn tree_view_mermaid_11_16_annotations_render_svg_dom() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"---
config:
  treeView:
    showIcons: true
    defaultIconPack: logos
    extensionIcons:
      ".tsx": react
---
treeView-beta
src/ :::highlight icon(folder) ## source directory
    App.tsx ## main component
    package.json icon(none)
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-11-16-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(svg.contains(r#"class="treeView-node-label treeView-node-dir highlight""#));
    assert!(svg.contains(".treeView-node-dir { font-weight: bold; }"));
    assert!(svg.contains(r#"class="treeView-highlight-bg""#));
    assert!(svg.contains(r#"class="treeView-node-description""#));
    assert!(svg.contains("source directory"));
    assert!(svg.contains("main component"));
    assert!(svg.contains(r#"id="tv-icon-tree-view-11-16-test-mermaid-treeview-folder""#));
    assert!(svg.contains(r#"id="tv-icon-tree-view-11-16-test-logos-react""#));
    assert!(!svg.contains("<use"));
    assert!(!svg.contains("package.json icon"));
    assert!(svg.contains(".treeView-node-icon"));
    assert!(svg.contains(".treeView-node-description"));
    assert!(svg.contains(".treeView-highlight-bg"));
}

#[test]
fn tree_view_security_level_controls_only_icon_use_nodes() {
    let input = "treeView-beta\nRoot icon(folder)\n";
    let options = SvgRenderOptions {
        diagram_id: Some("tree-view-security-level-test".to_string()),
        ..Default::default()
    };
    let strict_svg = render_tree_view_svg_with_options(input, options.clone());
    let loose_engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({ "securityLevel": "loose" }),
    ));
    let loose_svg = render_tree_view_svg_with_engine_and_environment(
        input,
        options,
        &loose_engine,
        &RenderEnvironment::deterministic(),
    );
    let strict_document = roxmltree::Document::parse(&strict_svg).expect("valid strict SVG");
    let loose_document = roxmltree::Document::parse(&loose_svg).expect("valid loose SVG");
    let symbol_id = "tv-icon-tree-view-security-level-test-mermaid-treeview-folder";
    let symbol_href = format!("#{symbol_id}");

    for document in [&strict_document, &loose_document] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some(symbol_id)),
            "both security levels retain the icon definition"
        );
    }
    assert!(
        strict_document
            .descendants()
            .all(|node| node.tag_name().name() != "use"),
        "strict rendering mirrors Mermaid's final DOMPurify pass"
    );
    assert!(loose_document.descendants().any(|node| {
        node.tag_name().name() == "use"
            && node.attribute(("http://www.w3.org/1999/xlink", "href"))
                == Some(symbol_href.as_str())
    }));

    let root_attribute = |document: &roxmltree::Document<'_>, name| {
        document
            .root_element()
            .attribute(name)
            .expect("root attribute")
            .to_string()
    };
    assert_eq!(
        root_attribute(&strict_document, "viewBox"),
        root_attribute(&loose_document, "viewBox")
    );
    let label_x = |document: &roxmltree::Document<'_>| {
        document
            .descendants()
            .find(|node| node.tag_name().name() == "text" && node.text() == Some("Root"))
            .and_then(|node| node.attribute("x"))
            .expect("label x")
            .to_string()
    };
    assert_eq!(label_x(&strict_document), label_x(&loose_document));
}

#[test]
fn tree_view_builtin_icons_render_at_fourteen_pixels_without_overlapping_labels() {
    let input = r##"treeView-beta
src/ icon(folder)
file.txt icon(file)
App.tsx icon(logos:react)
"##;

    let pack = br#"{
        "prefix":"mermaid-treeview",
        "icons":{
            "file":{"body":"<path data-icon=\"registry-override\"/>"},
            "folder":{"body":"<path data-icon=\"registry-override\"/>"}
        }
    }"#;
    let registry = IconRegistry::from_packs([IconPack::new(pack)]).unwrap();
    let environment = RenderEnvironment::deterministic().with_icon_registry(registry);
    let loose_engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({ "securityLevel": "loose" }),
    ));
    let svg = render_tree_view_svg_with_engine_and_environment(
        input,
        SvgRenderOptions {
            diagram_id: Some("tree-view-icon-size-test".to_string()),
            ..Default::default()
        },
        &loose_engine,
        &environment,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");

    for (icon, label) in [("folder", "src"), ("file", "file.txt")] {
        let symbol_id = format!("tv-icon-tree-view-icon-size-test-mermaid-treeview-{icon}");
        let symbol = document
            .descendants()
            .find(|node| node.attribute("id") == Some(symbol_id.as_str()))
            .expect("built-in icon definition");
        let icon_svg = symbol
            .children()
            .find(|node| node.is_element() && node.tag_name().name() == "svg")
            .expect("built-in icon uses a size-constrained SVG viewport");

        assert_eq!(icon_svg.attribute("width"), Some("14"));
        assert_eq!(icon_svg.attribute("height"), Some("14"));
        assert_eq!(icon_svg.attribute("viewBox"), Some("0 0 24 24"));

        let href = format!("#{symbol_id}");
        let icon_use = document
            .descendants()
            .find(|node| {
                node.tag_name().name() == "use"
                    && node.attribute(("http://www.w3.org/1999/xlink", "href"))
                        == Some(href.as_str())
            })
            .expect("icon use node");
        let label_node = document
            .descendants()
            .find(|node| node.tag_name().name() == "text" && node.text() == Some(label))
            .expect("icon label node");
        let icon_right = icon_use
            .attribute("x")
            .expect("icon x")
            .parse::<f64>()
            .expect("numeric icon x")
            + 14.0;
        let label_x = label_node
            .attribute("x")
            .expect("label x")
            .parse::<f64>()
            .expect("numeric label x");

        assert_eq!(label_x - icon_right, 4.0);
    }
    assert!(!svg.contains("registry-override"), "{svg}");

    let third_party_symbol = document
        .descendants()
        .find(|node| node.attribute("id") == Some("tv-icon-tree-view-icon-size-test-logos-react"))
        .expect("third-party fallback icon definition");
    let fallback_svg = third_party_symbol
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "svg")
        .expect("missing icon uses the standard fallback SVG");
    assert_eq!(fallback_svg.attribute("width"), Some("14"));
    assert_eq!(fallback_svg.attribute("height"), Some("14"));
    assert_eq!(fallback_svg.attribute("viewBox"), Some("0 0 80 80"));
    assert!(
        fallback_svg
            .children()
            .any(|node| node.is_element() && node.tag_name().name() == "g")
    );
}

#[test]
fn tree_view_registry_icons_preserve_viewbox_and_empty_body_semantics() {
    let pack = br#"{
        "prefix":"test",
        "icons":{
            "rocket":{
                "body":"<path data-icon=\"tree-view-registry\" d=\"M2 3H34V21H2z\"/>",
                "left":2,
                "top":3,
                "width":32,
                "height":18
            },
            "empty":{"body":""}
        }
    }"#;
    let registry = IconRegistry::from_packs([IconPack::new(pack)]).unwrap();
    let environment = RenderEnvironment::deterministic().with_icon_registry(registry);
    let svg = render_tree_view_svg_with_environment(
        "treeView-beta\nRoot\n    Rocket icon(test:rocket)\n    Rocket Again icon(test:rocket)\n    Missing icon(test:missing)\n    Empty icon(test:empty)\n",
        SvgRenderOptions {
            diagram_id: Some("tree-view-registry-test".to_string()),
            ..Default::default()
        },
        &environment,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");

    let rocket_symbol = document
        .descendants()
        .find(|node| node.attribute("id") == Some("tv-icon-tree-view-registry-test-test-rocket"))
        .expect("registry icon symbol");
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                node.attribute("id") == Some("tv-icon-tree-view-registry-test-test-rocket")
            })
            .count(),
        1
    );
    let rocket_svg = rocket_symbol
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "svg")
        .expect("registry icon SVG");
    assert_eq!(rocket_svg.attribute("width"), Some("14"));
    assert_eq!(rocket_svg.attribute("height"), Some("14"));
    assert_eq!(rocket_svg.attribute("viewBox"), Some("2 3 32 18"));
    assert!(
        rocket_svg
            .descendants()
            .any(|node| node.attribute("data-icon") == Some("tree-view-registry"))
    );

    assert_unknown_tree_view_icon(&document, "tv-icon-tree-view-registry-test-test-missing");

    let empty_symbol = document
        .descendants()
        .find(|node| node.attribute("id") == Some("tv-icon-tree-view-registry-test-test-empty"))
        .expect("empty registry icon symbol");
    let empty_svg = empty_symbol
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "svg")
        .expect("an explicitly empty registry icon still resolves");
    assert_eq!(empty_svg.attribute("viewBox"), Some("0 0 16 16"));
    assert_eq!(
        empty_svg
            .children()
            .filter(|node| node.is_element())
            .count(),
        0
    );
}

#[test]
fn tree_view_missing_icon_without_registry_uses_unknown_icon() {
    let svg = render_tree_view_svg_with_options(
        "treeView-beta\nRoot icon(test:missing)\n",
        SvgRenderOptions {
            diagram_id: Some("tree-view-no-registry-test".to_string()),
            ..Default::default()
        },
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");
    assert_unknown_tree_view_icon(&document, "tv-icon-tree-view-no-registry-test-test-missing");
}

fn assert_unknown_tree_view_icon(document: &roxmltree::Document<'_>, symbol_id: &str) {
    let symbol = document
        .descendants()
        .find(|node| node.attribute("id") == Some(symbol_id))
        .expect("missing icon symbol");
    let icon_svg = symbol
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "svg")
        .expect("unknown icon SVG");
    assert_eq!(icon_svg.attribute("width"), Some("14"));
    assert_eq!(icon_svg.attribute("height"), Some("14"));
    assert_eq!(icon_svg.attribute("viewBox"), Some("0 0 80 80"));
    assert_eq!(
        icon_svg
            .descendants()
            .find(|node| node.tag_name().name() == "tspan")
            .and_then(|node| node.text()),
        Some("?")
    );
}

#[test]
fn tree_view_root_highlight_visual_bounds_fit_inside_viewbox() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"treeView-beta
root/ :::highlight
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let tree_view = layout_tree_view(input, &RenderEnvironment::deterministic());
    let content_width = tree_view
        .nodes
        .iter()
        .map(|node| node.x + node.width)
        .fold(0.0, f64::max);

    assert_eq!(tree_view.total_width, content_width + 10.0);

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-root-highlight-test".to_string()),
            ..Default::default()
        },
        session,
    );
    assert_tree_view_highlights_fit_viewbox(&svg);
}

#[test]
fn tree_view_multiple_highlights_follow_upstream_width_growth() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"treeView-beta
root/ :::highlight
    child/ :::highlight
        leaf.txt
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let tree_view = layout_tree_view(input, &RenderEnvironment::deterministic());
    let content_width = tree_view
        .nodes
        .iter()
        .map(|node| node.x + node.width)
        .fold(0.0, f64::max);
    let highlighted_nodes = tree_view
        .nodes
        .iter()
        .filter(|node| {
            node.css_class
                .as_deref()
                .is_some_and(|class| class.split_whitespace().any(|part| part == "highlight"))
        })
        .collect::<Vec<_>>();

    assert_eq!(highlighted_nodes.len(), 2);
    assert_eq!(tree_view.total_width, content_width + 20.0);

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-multiple-highlights-test".to_string()),
            ..Default::default()
        },
        session,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");
    let highlight_rects = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("treeView-highlight-bg"))
        .collect::<Vec<_>>();
    let mut width_before_highlight = content_width;
    for (node, rect) in highlighted_nodes.into_iter().zip(highlight_rects) {
        let actual_width = rect
            .attribute("width")
            .expect("highlight width")
            .parse::<f64>()
            .expect("numeric highlight width");
        let expected_width = width_before_highlight - node.x + 8.0;
        assert!((actual_width - expected_width).abs() < 1e-9);
        width_before_highlight += 10.0;
    }
    assert_eq!(width_before_highlight, tree_view.total_width);
    assert_tree_view_highlights_fit_viewbox(&svg);
}

fn assert_tree_view_highlights_fit_viewbox(svg: &str) {
    let document = roxmltree::Document::parse(svg).expect("valid TreeView SVG");
    let view_box = document
        .root_element()
        .attribute("viewBox")
        .expect("TreeView viewBox")
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    let view_box_right = view_box[0] + view_box[2];

    for rect in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("treeView-highlight-bg"))
    {
        let x = rect
            .attribute("x")
            .expect("highlight x")
            .parse::<f64>()
            .expect("numeric highlight x");
        let width = rect
            .attribute("width")
            .expect("highlight width")
            .parse::<f64>()
            .expect("numeric highlight width");
        let visual_right = x + width + 0.5;
        assert!(
            visual_right <= view_box_right,
            "highlight right edge {visual_right} exceeds viewBox right edge {view_box_right}"
        );
    }
}

#[test]
fn tree_view_trailing_slash_only_marks_directory_labels() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"treeView-beta
src/ :::directory-probe
    main.rs :::file-probe
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-directory-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(
        svg.contains(r#"class="treeView-node-label treeView-node-dir directory-probe""#),
        "trailing-slash directory should receive the upstream directory class: {svg}"
    );
    assert!(svg.contains(r#"class="treeView-node-label file-probe""#));
    assert!(!svg.contains(r#"treeView-node-dir file-probe"#));
}

#[test]
fn tree_view_layout_measures_directory_labels_with_bold_style() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let layout = layout_tree_view(
        "treeView-beta\nverylongdirectoryname/ :::directory-probe\n",
        &RenderEnvironment::deterministic(),
    );
    let directory = layout
        .nodes
        .iter()
        .find(|node| node.css_class.as_deref() == Some("directory-probe"))
        .expect("directory node");
    let style = TextStyle {
        font_size: layout.label_font_size,
        font_weight: Some("bold".to_string()),
        ..Default::default()
    };
    let expected = session
        .text_measurer(TextMeasurementPhase::SvgBBox)
        .measure_svg_raw_text_bbox_width_px(&directory.name, &style);

    assert_eq!(directory.label_width, expected);
}

#[test]
fn tree_view_layout_measures_descriptions_with_italic_style() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let layout = layout_tree_view(
        "treeView-beta\nfile.txt ## a long slanted description\n",
        &RenderEnvironment::deterministic(),
    );
    let node = layout
        .nodes
        .iter()
        .find(|node| node.description.is_some())
        .expect("described node");
    let description = node.description.as_deref().expect("description");
    let style = TextStyle {
        font_size: layout.label_font_size,
        font_style: Some("italic".to_string()),
        ..Default::default()
    };
    let expected = session
        .text_measurer(TextMeasurementPhase::SvgBBox)
        .measure_svg_raw_text_bbox_width_px(description, &style);

    assert_eq!(node.description_width, Some(expected));
}

#[test]
fn tree_view_layout_routes_direct_text_bbox_height_through_the_host() {
    let host = Arc::new(TreeViewBBoxHost::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.tree-view-bbox-host").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(
            identity,
            host.clone(),
            [TextMeasurementPhase::SvgBBox],
        ),
    );
    let layout = layout_tree_view("treeView-beta\nfile.txt\n", &environment);

    assert!(layout.nodes.iter().all(|node| node.label_height == 31.0));
    assert!(layout.nodes.iter().all(|node| node.height == 41.0));
    assert!(
        host.operations
            .lock()
            .unwrap()
            .contains(&TextMeasurementOperation::RawBBoxHeight)
    );
}

#[test]
fn tree_view_fixed_size_root_keeps_width_and_height() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let input = r##"---
config:
  treeView:
    useMaxWidth: false
---
treeView-beta
"Root"
    "Child"
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "treeView");

    let svg = render_parsed_tree_view_svg(
        parsed,
        SvgRenderOptions {
            diagram_id: Some("tree-view-fixed-test".to_string()),
            ..Default::default()
        },
        session,
    );

    assert!(!svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"<svg id="tree-view-fixed-test" width=""#));
    assert!(svg.contains(r#"" height=""#));
    assert!(svg.contains(r#"style="background-color: white;" viewBox="-0.5 0 "#));
    assert!(!svg.contains("max-width:"));
}

#[test]
fn tree_view_public_layout_accepts_max_allowed_chain() {
    let mut input = String::from("treeView-beta\n");
    for depth in 0..MAX_DIAGRAM_NESTING_DEPTH {
        input.push_str(&" ".repeat(depth));
        input.push('"');
        input.push_str(&format!("n{depth}"));
        input.push_str("\"\n");
    }

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata().diagram_type, "treeView");

    let trusted_environment = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input());
    let tree_view = layout_tree_view(&input, &trusted_environment);

    assert_eq!(tree_view.nodes.len(), MAX_DIAGRAM_NESTING_DEPTH + 1);
    assert_eq!(
        tree_view.nodes.first().map(|node| node.name.as_str()),
        Some("/")
    );
    let expected_last = format!("n{}", MAX_DIAGRAM_NESTING_DEPTH - 1);
    assert_eq!(
        tree_view.nodes.last().map(|node| node.name.as_str()),
        Some(expected_last.as_str())
    );
}
