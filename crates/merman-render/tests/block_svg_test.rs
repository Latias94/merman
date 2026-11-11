mod common;

use base64::Engine as _;
use common::legacy_init_theme_compat_engine;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    OrdinalSelector, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};
use regex::Regex;

fn independent_block_presentation(
    model: merman_model::families::block::BlockDiagramModel,
) -> merman_model::presentation::PresentationDiagram {
    independent_block_presentation_with_config(
        model,
        merman_model::MermaidConfig::empty_object(),
        None,
    )
}

fn independent_block_presentation_with_config(
    model: merman_model::families::block::BlockDiagramModel,
    config: merman_model::MermaidConfig,
    theme: Option<&DiagramTheme>,
) -> merman_model::presentation::PresentationDiagram {
    use merman_model::OperationControl;
    use merman_model::families::block::BlockDiagramRenderModel;
    use merman_model::presentation::{
        AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
    };
    use merman_model::resources::InputResourcePolicy;

    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "block".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
            theme_recipe: theme.map(|theme| theme.compatibility_recipe().clone()),
        },
        RenderSemanticModel::Block(BlockDiagramRenderModel::from_semantic_model(model)),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("independent Block construction is active")
    .expect("small Block model passes construction resource admission")
}

fn independent_block_tree(
    children: &[&[usize]],
) -> merman_model::families::block::BlockDiagramModel {
    use merman_model::families::block::{BlockDiagramModel, BlockNode};

    BlockDiagramModel {
        root: Some(0),
        blocks_flat: children
            .iter()
            .enumerate()
            .map(|(index, children)| BlockNode {
                id: if index == 0 {
                    "root".to_string()
                } else {
                    format!("node{index}")
                },
                label: "Repeated label".to_string(),
                block_type: if children.is_empty() {
                    "square".to_string()
                } else {
                    "composite".to_string()
                },
                children: children.to_vec(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn block_direct_input_rejects_repeated_child_indices_before_layout_expansion() {
    for model in [
        independent_block_tree(&[&[1, 1], &[]]),
        independent_block_tree(&[&[1, 2], &[3], &[3], &[]]),
    ] {
        let parsed = independent_block_presentation(model);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = family::prepare(&parsed, &LayoutOptions::default(), session)
            .err()
            .expect("a flat child record cannot own two layout occurrences");
        assert!(matches!(
            error,
            merman_render::Error::InvalidModel { ref message }
                if message.contains("repeated Block child index")
        ));
    }
}

#[test]
fn block_direct_input_rejects_invalid_column_spans() {
    for second_span in [-1.0, 1.5, f64::NAN, f64::NEG_INFINITY] {
        let mut model = independent_block_tree(&[&[1, 2], &[], &[]]);
        model.blocks_flat[1].width_in_columns = Some(1.0);
        model.blocks_flat[2].width_in_columns = Some(second_span);
        let parsed = independent_block_presentation(model);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = family::prepare(&parsed, &LayoutOptions::default(), session)
            .err()
            .expect("invalid direct spans must fail before grid arithmetic");
        assert!(matches!(
            error,
            merman_render::Error::InvalidModel { ref message }
                if message.contains("Block column span")
        ));
    }
}

#[test]
fn block_direct_input_preserves_distinct_records_and_supported_column_spans() {
    for (first_span, second_span) in [(1.0, 3.0), (9223372036854775808.0, 1.0)] {
        let mut model = independent_block_tree(&[&[1, 2], &[], &[]]);
        model.blocks_flat[1].id = "Repeated".to_string();
        model.blocks_flat[2].id = "Repeated".to_string();
        model.blocks_flat[1].width_in_columns = Some(first_span);
        model.blocks_flat[2].width_in_columns = Some(second_span);
        let parsed = independent_block_presentation(model);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
            .expect("distinct flat records and JavaScript Number spans remain valid");
        let projection = artifact.layout_json().expect("project finite Block layout");
        let layout: merman_render::model::BlockDiagramLayout =
            serde_json::from_value(projection["layout"]["BlockDiagram"].clone()).unwrap();
        assert_eq!(layout.nodes.len(), 2);
        for node in &layout.nodes {
            assert_eq!(node.id, "Repeated");
            assert!(
                [node.x, node.y, node.width, node.height]
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
    }
}

#[test]
fn block_direct_input_validation_preserves_operation_error_priority() {
    use merman_model::OperationControl;

    let parsed = independent_block_presentation(independent_block_tree(&[&[1, 1], &[]]));
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(merman_render::Error::Cancelled(_))
    ));

    let control = OperationControl::new();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxModelItems, 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session_with_control(control.clone())
        .unwrap();
    let error = family::plan_render(&parsed, &session).err().unwrap();
    let merman_render::Error::ResourceLimitExceeded(first) = error else {
        panic!("expected model resource admission before Block validation, got {error:?}");
    };
    assert_eq!(first.limit, ResourceLimitId::MaxModelItems.as_str());
    control.cancel();
    let error = family::prepare(&parsed, &LayoutOptions::default(), session)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        merman_render::Error::ResourceLimitExceeded(replayed) if replayed == first
    ));
}

#[test]
fn block_large_columns_preserve_the_auto_grid_without_integer_rejection() {
    let automatic = render_block_svg_from_text("block\ncolumns auto\nA\nB\n");
    for columns in [
        "9007199254740993",
        "9223372036854775808",
        "18446744073709551615",
    ] {
        let svg = render_block_svg_from_text(&format!("block\ncolumns {columns}\nA\nB\n"));
        assert_eq!(svg, automatic, "{columns}");
    }
}

#[test]
fn block_zero_and_overflowing_counts_keep_the_upstream_render_stage() {
    let engine = MermaidEngine::new();
    for columns in ["0".to_string(), "9".repeat(309)] {
        let error = try_render_block_svg_from_text_with_engine(
            &engine,
            &format!("block\ncolumns {columns}\nA\n"),
        )
        .expect_err("the selected layout rejects zero and nonfinite columns");
        assert!(
            matches!(error, merman_render::Error::InvalidModel { ref message } if message == "Columns must be an integer !== 0.")
        );
        try_render_block_svg_from_text_with_engine(&engine, &format!("block\ncolumns {columns}\n"))
            .expect("an empty composite never calculates a column position");
    }
    let token = "9".repeat(309);
    let error =
        try_render_block_svg_from_text_with_engine(&engine, &format!("block\nA:{token}\nB\n"))
            .expect_err("a subsequent position becomes nonfinite");
    assert!(
        matches!(error, merman_render::Error::InvalidModel { ref message } if message == "Position must be a non-negative integer.Infinity")
    );
    for source in [
        "block\nA:0\nB\n".to_string(),
        "block\nA{{\"A\"}}:0\nB\n".to_string(),
        "block\nA[(\"A\")]:0\nB\n".to_string(),
        format!("block\nA:{token}\n"),
        format!("block\ncolumns 2\nA:{token}\nB\n"),
    ] {
        let error = try_render_block_svg_from_text_with_engine(&engine, &source)
            .expect_err("nonfinite layout must retain the shared finite-root SVG contract");
        assert!(
            matches!(error, merman_render::Error::InvalidModel { ref message }
            if message.starts_with("root SVG viewBox ") && message.contains("must be finite")),
            "{source}"
        );
    }
}

fn render_block_svg_from_text(text: &str) -> String {
    let engine = MermaidEngine::new();
    render_block_svg_from_text_with_engine(&engine, text)
}

fn render_block_svg_from_text_with_engine(engine: &MermaidEngine, text: &str) -> String {
    try_render_block_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
    .expect("svg render ok")
}

fn try_render_block_svg_from_text_with_engine(
    engine: &MermaidEngine,
    text: &str,
) -> merman_render::Result<String> {
    try_render_block_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
}

fn try_render_block_svg_from_text_with_engine_and_policy(
    engine: &MermaidEngine,
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;

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

fn root_view_box(document: &roxmltree::Document<'_>) -> (f64, f64, f64, f64) {
    let root = document.root_element();
    let values = root
        .attribute("viewBox")
        .expect("root viewBox")
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4, "viewBox must contain four components");
    (values[0], values[1], values[2], values[3])
}

fn render_block_geometry(config: serde_json::Value, source: &str) -> (String, serde_json::Value) {
    let engine = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Block geometry")
        .expect("detect Block geometry")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Block geometry");
    let layout =
        artifact.layout_json().expect("project Block geometry")["layout"]["BlockDiagram"].clone();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Block geometry")
        .svg()
        .to_owned();
    (svg, layout)
}

fn block_node_rect<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    node_id: &str,
) -> roxmltree::Node<'a, 'input> {
    document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .expect("Block node")
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("Block rectangle terminal")
}

fn block_shape_geometry<'a>(layout: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    layout["shapeGeometries"]
        .as_array()
        .expect("Block shape geometries")
        .iter()
        .find(|geometry| geometry["id"] == id)
        .expect("Block node geometry")
}

fn block_stylesheet_text(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("Block stylesheet XML");
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Block logical stylesheet")
        .to_owned()
}

// Check only Block's generated class selectors against the actual label DOM. These fixtures
// have one source color, so no general stylesheet parser or inferred ancestor cascade is needed.
fn assert_block_source_class_reaches_label_terminal(
    document: &roxmltree::Document<'_>,
    terminal: roxmltree::Node<'_, '_>,
    class: &str,
    property: &str,
    color: &str,
) {
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap();
    let diagram_id = document.root_element().attribute("id").unwrap();
    let has_class = |node: roxmltree::Node<'_, '_>| {
        node.attribute("class")
            .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
    };
    if terminal.has_tag_name("p") {
        assert_eq!(property, "color");
        assert!(terminal.parent_element().is_some_and(has_class));
        assert!(css.contains(&format!(
            "#{diagram_id} .{class}>*{{color:{color}!important;}}"
        )));
    } else if property == "color" {
        let word = terminal
            .descendants()
            .find(|node| node.has_tag_name("tspan") && node.children().any(|node| node.is_text()))
            .expect("visible native label word");
        assert!(word.ancestors().skip(1).any(has_class));
        assert!(css.contains(&format!(
            "#{diagram_id} .{class} tspan{{fill:{color}!important;}}"
        )));
    } else {
        assert_eq!(property, "fill");
        assert!(terminal.descendants().any(|node| {
            node.parent_element().is_some_and(has_class)
                && (node.has_tag_name("text") || node.has_tag_name("tspan"))
        }));
        assert!(css.contains(&format!(
            "#{diagram_id} .{class}>*{{fill:{color}!important;}}"
        )));
    }
}

fn numeric_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name)
        .unwrap_or_else(|| panic!("missing {name} attribute"))
        .parse()
        .unwrap_or_else(|_| panic!("numeric {name} attribute"))
}

fn matched_block_node_stroke_width(document: &roxmltree::Document<'_>, id: &str) -> String {
    let shell = block_node_rect(document, id);
    assert!(shell.ancestors().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "node"))
    }));
    let selector = format!("#merman .node {}", shell.tag_name().name());
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Block stylesheet");
    css.split('}')
        .filter_map(|rule| rule.split_once('{'))
        .find(|(selectors, _)| selectors.split(',').any(|part| part.trim() == selector))
        .expect("CSS rule matching the actual Block shell")
        .1
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .find(|(property, _)| property.trim() == "stroke-width")
        .expect("matched node stroke-width declaration")
        .1
        .trim()
        .to_owned()
}

fn parse_translate(raw: &str) -> (f64, f64) {
    let captures = Regex::new(
        r"^translate\(\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*\)$",
    )
    .expect("valid transform regex")
    .captures(raw)
    .expect("translate(x, y) transform");
    (
        captures[1].parse().expect("numeric translate x"),
        captures[2].parse().expect("numeric translate y"),
    )
}

fn global_foreign_object_bounds(foreign_object: roxmltree::Node<'_, '_>) -> (f64, f64, f64, f64) {
    let width = foreign_object
        .attribute("width")
        .expect("foreignObject width")
        .parse::<f64>()
        .expect("numeric foreignObject width");
    let height = foreign_object
        .attribute("height")
        .expect("foreignObject height")
        .parse::<f64>()
        .expect("numeric foreignObject height");
    let x = foreign_object
        .attribute("x")
        .unwrap_or("0")
        .parse::<f64>()
        .expect("numeric foreignObject x");
    let y = foreign_object
        .attribute("y")
        .unwrap_or("0")
        .parse::<f64>()
        .expect("numeric foreignObject y");

    let (mut tx, mut ty) = (x, y);
    for ancestor in foreign_object.ancestors().filter(|node| node.is_element()) {
        if let Some(transform) = ancestor.attribute("transform") {
            let (ancestor_x, ancestor_y) = parse_translate(transform);
            tx += ancestor_x;
            ty += ancestor_y;
        }
    }
    (tx, ty, tx + width, ty + height)
}

fn global_path_geometry_bounds(path: roxmltree::Node<'_, '_>) -> (f64, f64, f64, f64) {
    let d = path.attribute("d").expect("path data");
    let fragment = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="{d}"/></svg>"#);
    let bounds = merman_render::svg::debug_svg_emitted_bounds(&fragment)
        .expect("path geometry bounds")
        .bounds;
    let (mut tx, mut ty) = (0.0, 0.0);
    for ancestor in path.ancestors().filter(|node| node.is_element()) {
        if let Some(transform) = ancestor.attribute("transform") {
            let (ancestor_x, ancestor_y) = parse_translate(transform);
            tx += ancestor_x;
            ty += ancestor_y;
        }
    }
    (
        bounds.min_x + tx,
        bounds.min_y + ty,
        bounds.max_x + tx,
        bounds.max_y + ty,
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

fn block_node_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile Block node fill theme")
}

fn block_node_fill_and_stroke_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(fill)
                            .with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile Block node fill and stroke theme")
}

fn block_node_fill_with_ordinal_palette_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default().with_fill(fill),
                        )
                        .for_family(DiagramFamilyId::BLOCK),
                    )
                    .with_ordinal_palette(
                        ThemeTarget::Node,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#123456").expect("valid palette color")
                        ])
                        .expect("non-empty Block node palette"),
                    ),
            ),
        )
        .expect("compile Block node fill and ordinal palette theme")
}

fn block_node_ordinal_palette_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    OrdinalPalette::new([
                        ThemeColorValue::parse("#123456").expect("valid palette color")
                    ])
                    .expect("non-empty Block node palette"),
                ),
            ),
        )
        .expect("compile Block node ordinal palette theme")
}

fn block_typography_theme(font_family: &str, font_size_px: f32) -> DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single(font_family).expect("valid Block font stack"))
        .with_font_size_px(font_size_px)
        .expect("valid Block font size");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::BLOCK, typography),
        ))
        .expect("compile Block typography theme")
}

fn render_block_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
) -> family::RenderedFamilySvg {
    render_block_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn render_block_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    requirement: ThemePortabilityRequirement,
) -> family::RenderedFamilySvg {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Block")
        .expect("detect themed Block")
        .into_presentation();
    render_block_presentation_with_theme(&parsed, theme, requirement)
}

fn render_block_presentation_with_theme(
    parsed: &merman_model::presentation::PresentationDiagram,
    theme: &DiagramTheme,
    requirement: ThemePortabilityRequirement,
) -> family::RenderedFamilySvg {
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(theme)
        .expect("begin themed Block session");

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

fn block_node_label_theme(patch: ThemeStylePatch) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::NodeLabel, patch).for_family(DiagramFamilyId::BLOCK),
            )),
        )
        .unwrap()
}

fn block_generic_text_theme(patch: ThemeStylePatch) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::Text, patch).for_family(DiagramFamilyId::BLOCK),
            )),
        )
        .unwrap()
}

#[test]
fn block_generic_text_paint_is_bound_to_node_text_terminals() {
    let theme = block_generic_text_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#0f8a6e").unwrap()),
    );
    let rendered =
        render_block_with_theme_and_engine("block\n A[\"Alpha\"]\n", &theme, MermaidEngine::new());
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let node = document
        .descendants()
        .find(|n| n.attribute("id") == Some("block-theme-A"))
        .unwrap();
    assert!(
        node.descendants()
            .any(|n| { n.attribute("style").unwrap_or("").contains("#0f8a6e") })
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
}

#[test]
fn block_generic_text_stroke_keeps_a_residual_with_or_without_fill() {
    for fill in [false, true] {
        let mut patch =
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap());
        if fill {
            patch = patch.with_fill(CanvasPaint::solid("#0f8a6e").unwrap());
        }
        let theme = block_generic_text_theme(patch);
        let source = "block\n A[\"Alpha\"]\n";
        let rendered = render_block_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap();
        let result = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
        assert!(
            matches!(
                result,
                Err(merman_render::Error::UnverifiedFamilyTheme {
                    family_id: DiagramFamilyId::BLOCK,
                    residual_count: 1,
                })
            ),
            "Text.stroke must remain unproved even when fill is consumed"
        );
    }
}

#[test]
fn block_node_label_paint_is_bound_to_node_terminals() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\"Alpha\"] -- \"Edge\" --> B[\"Beta\"]\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": html}),
                ),
            ),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let stylesheet = document
            .descendants()
            .find(|n| n.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(
            !stylesheet.contains("#b316cd"),
            "node paint must not replace shared label CSS"
        );
        for id in ["block-theme-A", "block-theme-B"] {
            let node = document
                .descendants()
                .find(|n| n.attribute("id") == Some(id))
                .unwrap();
            let terminal = node
                .descendants()
                .find(|n| n.has_tag_name(if html { "p" } else { "text" }))
                .unwrap();
            assert!(
                terminal
                    .attribute("style")
                    .unwrap_or("")
                    .contains("#b316cd")
            );
        }
        let edge = document
            .descendants()
            .find(|node| node.attribute("class") == Some("edgeLabel"))
            .expect("edge label terminal");
        assert!(
            !edge
                .descendants()
                .any(|node| node.attribute("style").unwrap_or("").contains("#b316cd"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
    }
}

#[test]
fn block_empty_node_label_paint_is_not_applicable() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\" \"]\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": html}),
                ),
            ),
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_html_block_label_colors_do_not_certify_an_empty_paragraph() {
    let color = "#b316cd";
    for target in [ThemeTarget::NodeLabel, ThemeTarget::ClusterLabel] {
        let theme = block_cluster_theme([ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
        )]);
        for mixed in [false, true] {
            for source_color in [false, true] {
                let label = if mixed {
                    "Before<div>Alpha</div>"
                } else {
                    "<div>Alpha</div>"
                };
                let (mut source, id) = if target == ThemeTarget::ClusterLabel {
                    (
                        format!("block\nblock:G[\"{label}\"]\nC[\"Child\"]\nend\n"),
                        "G",
                    )
                } else {
                    (format!("block\nA[\"{label}\"]\n"), "A")
                };
                if source_color {
                    source.push_str(&format!("style {id} color:#123456\n"));
                }
                let engine = || {
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({"htmlLabels": true, "securityLevel": "strict"}),
                        ),
                    )
                };
                let rendered = render_block_with_theme_requirement(
                    &source,
                    &theme,
                    engine(),
                    ThemePortabilityRequirement::BestEffort,
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let node = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(format!("block-theme-{id}").as_str()))
                    .unwrap();
                let visible = node
                    .descendants()
                    .find(|node| node.has_tag_name("div") && node.text() == Some("Alpha"))
                    .expect("the real visible HTML block must remain in the label");
                assert!(
                    !visible.ancestors().any(|node| node.has_tag_name("p")),
                    "HTML5 recovery places the visible block outside p"
                );
                assert!(
                    !node.descendants().any(|node| node
                        .attribute("style")
                        .is_some_and(|style| style.contains(color))),
                    "an unconsumed paint must not color an empty p"
                );
                if source_color {
                    assert!(
                        visible.ancestors().any(|node| node
                            .attribute("style")
                            .is_some_and(|style| style.contains("color:#123456"))),
                        "the visible div retains its actual inherited source color"
                    );
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                let source_owned = source_color && !mixed;
                assert_eq!(evidence.applied_count(), 0);
                assert_eq!(evidence.not_applicable_count(), usize::from(source_owned));
                assert_eq!(evidence.theme_residual_count(), usize::from(!source_owned));

                let parsed = common::install_parse_compatibility(&theme, engine())
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let result =
                    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
                        .unwrap()
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
                if source_owned {
                    assert!(
                        result.is_ok(),
                        "fully source-owned block text remains not applicable"
                    );
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(merman_render::Error::UnverifiedFamilyTheme {
                                family_id: DiagramFamilyId::BLOCK,
                                residual_count: 1,
                            })
                        ),
                        "unconsumed visible HTML must fail strict theme validation"
                    );
                }
            }
        }
    }
}

#[test]
fn block_label_color_requires_all_effective_terminals_to_consume_the_rule() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    let rendered = render_block_with_theme_requirement(
        "block\nA[\"Alpha\"]\nB[\"<div>Beta</div>\"]\n",
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"htmlLabels": true, "securityLevel": "strict"}),
            ),
        ),
        ThemePortabilityRequirement::BestEffort,
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let paragraph = document
        .descendants()
        .find(|node| node.has_tag_name("p") && node.text() == Some("Alpha"))
        .unwrap();
    assert_eq!(paragraph.attribute("style"), Some("color:#b316cd;"));
    let block = document
        .descendants()
        .find(|node| node.has_tag_name("div") && node.text() == Some("Beta"))
        .unwrap();
    assert!(!block.ancestors().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains("#b316cd"))
    }));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(
        evidence.applied_count(),
        0,
        "one successful paragraph cannot certify the sibling block"
    );
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn block_node_label_inline_source_color_follows_the_terminal_cascade() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        for important in [false, true] {
            let source_value = if important {
                "#123456 !important"
            } else {
                "#123456"
            };
            let source = format!("block\n A[\"Alpha\"]\n style A color:{source_value}\n");
            let rendered = render_block_with_theme_and_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"htmlLabels": html}),
                    ),
                ),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|node| node.attribute("id") == Some("block-theme-A"))
                .unwrap();
            let terminal = node
                .descendants()
                .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
                .unwrap();
            assert_eq!(
                terminal.attribute("style"),
                Some(
                    if html {
                        "color:#b316cd;".to_owned()
                    } else {
                        format!("fill:{source_value};")
                    }
                    .as_str()
                ),
            );
            if html {
                let span = terminal.parent_element().unwrap();
                assert!(span.has_tag_name("span"));
                assert_eq!(
                    span.attribute("style"),
                    Some(format!("color:{source_value};").as_str())
                );
                assert!(block_stylesheet_text(rendered.svg()).contains("#block-theme p{fill:"));
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(html));
            assert_eq!(evidence.not_applicable_count(), usize::from(!html));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn block_node_label_overwritten_rule_has_no_residual() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#abcdef").unwrap()),
                        )
                        .with_variant(ThemeVariant::Default)
                        .for_family(DiagramFamilyId::BLOCK),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        )
                        .for_family(DiagramFamilyId::BLOCK),
                    ),
            ),
        )
        .unwrap();
    let rendered =
        render_block_with_theme_and_engine("block\n A[\"Alpha\"]\n", &theme, MermaidEngine::new());
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_node_label_mixed_rule_cannot_certify_unconsumed_stroke() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
    );
    let rendered = render_block_with_theme_requirement(
        "block\n A[\"Alpha\"]\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert!(
        evidence.theme_residual_count() > 0
            || evidence.accounted_count() < evidence.required_count()
    );
}

#[test]
fn block_node_label_mixed_rule_is_rejected_in_strict_mode() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
    );
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync("block\n A[\"Alpha\"]\n", ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    assert!(
        artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .is_err()
    );
}

#[test]
fn block_node_label_mixed_source_ownership_tracks_the_terminal_mode() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\"Alpha\"] B[\"Beta\"]\n style A color:#123456\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": html}),
                ),
            ),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for (id, color) in [
            ("block-theme-A", if html { "#b316cd" } else { "#123456" }),
            ("block-theme-B", "#b316cd"),
        ] {
            let node = document
                .descendants()
                .find(|n| n.attribute("id") == Some(id))
                .unwrap();
            let terminal = node
                .descendants()
                .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
                .unwrap();
            assert_eq!(
                terminal.attribute("style"),
                Some(format!("{}:{color};", if html { "color" } else { "fill" }).as_str()),
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
    }
}

#[test]
fn block_label_config_color_ownership_follows_the_actual_palette_consumer() {
    let theme = block_generic_text_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for (baseline, variables, applied_by_owner) in [
        (
            "default",
            serde_json::json!({"nodeTextColor": "#123456"}),
            [false, false],
        ),
        (
            "default",
            serde_json::json!({"primaryTextColor": "#123456"}),
            [true, true],
        ),
        (
            "default",
            serde_json::json!({"textColor": "#123456"}),
            [false, false],
        ),
        (
            "base",
            serde_json::json!({"nodeTextColor": "#123456"}),
            [false, false],
        ),
        (
            "base",
            serde_json::json!({"primaryTextColor": "#123456"}),
            [false, true],
        ),
        (
            "base",
            serde_json::json!({"textColor": "#123456"}),
            [true, true],
        ),
        (
            "dark",
            serde_json::json!({"nodeTextColor": "#123456"}),
            [false, false],
        ),
        (
            "dark",
            serde_json::json!({"primaryTextColor": "#123456"}),
            [true, true],
        ),
        (
            "dark",
            serde_json::json!({"textColor": "#123456"}),
            [false, false],
        ),
        (
            "redux",
            serde_json::json!({"nodeTextColor": "#123456"}),
            [false, false],
        ),
        (
            "redux",
            serde_json::json!({"primaryTextColor": "#123456"}),
            [false, true],
        ),
        (
            "redux",
            serde_json::json!({"textColor": "#123456"}),
            [true, true],
        ),
        (
            "default",
            serde_json::json!({"nodeTextColor": ""}),
            [true, true],
        ),
        (
            "default",
            serde_json::json!({"nodeTextColor": "", "textColor": "#123456"}),
            [false, false],
        ),
    ] {
        for html in [false, true] {
            for source_owned in [false, true] {
                let applied = applied_by_owner[usize::from(source_owned)];
                let mut config = common::legacy_init_theme_compat_config().as_value().clone();
                config["theme"] = serde_json::json!(baseline);
                config["htmlLabels"] = serde_json::json!(html);
                let source = if source_owned {
                    format!(
                        "---\nconfig: {}\n---\n{BLOCK_CLUSTER_SOURCE}",
                        serde_json::json!({"themeVariables": variables})
                    )
                } else {
                    config["themeVariables"] = variables.clone();
                    BLOCK_CLUSTER_SOURCE.to_owned()
                };
                let rendered = render_block_with_theme_and_engine(
                    &source,
                    &theme,
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                );
                for (key, value) in variables.as_object().unwrap() {
                    assert_eq!(
                        rendered
                            .metadata()
                            .effective_config
                            .get_str(&format!("themeVariables.{key}")),
                        value.as_str(),
                        "baseline={baseline} html={html} source_owned={source_owned} key={key}"
                    );
                }
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let typed_style = if html {
                    "color:#b316cd;"
                } else {
                    "fill:#b316cd;"
                };
                for id in ["block-theme-group", "block-theme-A", "block-theme-B"] {
                    let node = document
                        .descendants()
                        .find(|node| node.attribute("id") == Some(id))
                        .unwrap();
                    let label = node
                        .descendants()
                        .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
                        .unwrap();
                    assert_eq!(
                        label.attribute("style").filter(|style| !style.is_empty()),
                        applied.then_some(typed_style),
                        "baseline={baseline} html={html} source_owned={source_owned} variables={variables} id={id}"
                    );
                }
                if !applied {
                    assert!(block_stylesheet_text(rendered.svg()).contains(
                        "#block-theme .label text,#block-theme span,#block-theme p{fill:#123456;color:#123456;}"
                    ));
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.applied_count(), usize::from(applied));
                assert_eq!(evidence.not_applicable_count(), usize::from(!applied));
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn block_node_label_preserves_generic_text_author_order() {
    for node_last in [false, true] {
        let targets = if node_last {
            [ThemeTarget::Text, ThemeTarget::NodeLabel]
        } else {
            [ThemeTarget::NodeLabel, ThemeTarget::Text]
        };
        let rules = targets
            .into_iter()
            .fold(ThemeRuleSet::default(), |rules, target| {
                let color = if target == ThemeTarget::NodeLabel {
                    "#b316cd"
                } else {
                    "#123456"
                };
                rules.with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                )
            });
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let rendered = render_block_with_theme_requirement(
            "block\n A[\"Alpha\"]\n",
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = document
            .descendants()
            .find(|n| n.attribute("id") == Some("block-theme-A"))
            .unwrap();
        assert_eq!(
            node.descendants()
                .any(|n| n.attribute("style").unwrap_or("").contains("#b316cd")),
            node_last
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_node_label_class_color_owns_only_matching_text_terminals() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        for (source, owned) in [
            (
                "block\n A[\"Alpha\"]\n class A paint\n classDef paint color:#123456\n",
                true,
            ),
            (
                "block\n A[\"Alpha\"]\n classDef default color:#123456\n",
                true,
            ),
            (
                "block\n A[\"Alpha\"]\n class A plain\n classDef plain stroke:#123456\n classDef default color:#654321\n",
                false,
            ),
        ] {
            let owned = owned && !html;
            let rendered = render_block_with_theme_and_engine(
                source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"htmlLabels": html}),
                    ),
                ),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|n| n.attribute("id") == Some("block-theme-A"))
                .unwrap();
            let terminal = node
                .descendants()
                .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
                .unwrap();
            assert_eq!(
                terminal
                    .attribute("style")
                    .unwrap_or("")
                    .contains("#b316cd"),
                !owned
            );
            if owned {
                let class = if node
                    .attribute("class")
                    .unwrap()
                    .split_ascii_whitespace()
                    .any(|class| class == "paint")
                {
                    "paint"
                } else {
                    "default"
                };
                assert_block_source_class_reaches_label_terminal(
                    &document, terminal, class, "color", "#123456",
                );
            } else {
                assert_eq!(
                    terminal.attribute("style"),
                    Some(if html {
                        "color:#b316cd;"
                    } else {
                        "fill:#b316cd;"
                    })
                );
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(!owned));
            assert_eq!(evidence.not_applicable_count(), usize::from(owned));
        }
    }
}

#[test]
fn block_label_source_ownership_matches_structural_class_and_paint_domains() {
    use merman_model::families::block::BlockClassDef;

    let baseline_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let theme = block_cluster_theme([
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::ClusterLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
        ),
    ]);
    for html in [false, true] {
        for assigned in [false, true] {
            for (class, property, svg_owned, html_owned) in [
                ("block", "color", true, false),
                ("node", "color", true, false),
                ("flowchart-label", "color", true, false),
                ("label", "color", true, false),
                ("nodeLabel", "color", false, true),
                ("row", "color", true, false),
                ("text-outer-tspan", "color", true, false),
                ("text-inner-tspan", "color", false, false),
                ("block", "fill", false, false),
                ("node", "fill", false, false),
                ("flowchart-label", "fill", false, false),
                ("label", "fill", true, false),
                ("row", "fill", true, false),
                ("text-outer-tspan", "fill", true, false),
                ("nodeLabel", "fill", false, false),
                ("text-inner-tspan", "fill", false, false),
                ("node", "stroke", false, false),
                ("cluster", "color", false, false),
            ] {
                let assignment = if assigned {
                    "classDef plain stroke:#654321\nclass group,A,B plain\n"
                } else {
                    ""
                };
                let config = serde_json::json!({"htmlLabels":html});
                let render = |theme: &DiagramTheme| {
                    if class.contains('-') {
                        // Block source classDef IDs are alphanumeric/underscore only.
                        // These valid CSS classes belong to the independent model API.
                        let mut model = independent_block_tree(&[&[1, 3], &[2], &[], &[]]);
                        for (node, (id, label)) in model.blocks_flat.iter_mut().zip([
                            ("root", ""),
                            ("group", "Group"),
                            ("A", "Alpha"),
                            ("B", "Beta"),
                        ]) {
                            node.id = id.to_string();
                            node.label = label.to_string();
                            node.columns = Some(1.0);
                            if assigned && id != "root" {
                                node.classes.push("plain".into());
                            }
                        }
                        let mut definition = BlockClassDef::new(class);
                        definition.styles.push(format!("{property}:#123456"));
                        if property == "color" {
                            definition.text_styles.push("fill:#123456".into());
                        }
                        model.class_defs.insert(class.into(), definition);
                        if assigned {
                            let mut plain = BlockClassDef::new("plain");
                            plain.styles.push("stroke:#654321".into());
                            model.class_defs.insert("plain".into(), plain);
                        }
                        let presentation = independent_block_presentation_with_config(
                            model,
                            merman_model::MermaidConfig::from_value(config.clone()),
                            Some(theme),
                        );
                        assert!(
                            presentation
                                .metadata()
                                .effective_config
                                .theme_compatibility()
                                .matches_recipe(Some(theme.compatibility_recipe())),
                            "independent input must declare the same recipe as its render session"
                        );
                        render_block_presentation_with_theme(
                            &presentation,
                            theme,
                            ThemePortabilityRequirement::RequirePortable,
                        )
                    } else {
                        let source = format!(
                            "{BLOCK_CLUSTER_SOURCE}classDef {class} {property}:#123456\n{assignment}"
                        );
                        render_block_with_theme_and_engine(
                            &source,
                            theme,
                            MermaidEngine::new().with_site_config(
                                merman_core::presentation::SourcePresentationConfig::from_value(
                                    config.clone(),
                                ),
                            ),
                        )
                    }
                };
                let rendered = render(&theme);
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let owned = if html { html_owned } else { svg_owned };
                assert!(
                    block_stylesheet_text(rendered.svg())
                        .contains(&format!(".{class}>*{{{property}:#123456!important;}}"))
                );
                for (id, label, color) in [
                    ("block-theme-group", "Group", "#2468ac"),
                    ("block-theme-A", "Alpha", "#b316cd"),
                    ("block-theme-B", "Beta", "#b316cd"),
                ] {
                    let node = document
                        .descendants()
                        .find(|node| node.attribute("id") == Some(id))
                        .unwrap();
                    let classes = node.attribute("class").unwrap();
                    for expected in [
                        "node",
                        "flowchart-label",
                        if assigned { "plain" } else { "default" },
                    ] {
                        assert!(
                            classes
                                .split_ascii_whitespace()
                                .any(|class| class == expected)
                        );
                    }
                    let terminal = node
                        .descendants()
                        .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
                        .unwrap();
                    assert_eq!(
                        terminal
                            .descendants()
                            .filter(|node| node.is_text())
                            .filter_map(|node| node.text())
                            .collect::<String>(),
                        label,
                    );
                    assert_eq!(
                        terminal.attribute("style").unwrap_or("").contains(color),
                        !owned,
                        "{id}: class={class}, property={property}, html={html}, assigned={assigned}",
                    );
                    if owned {
                        assert_block_source_class_reaches_label_terminal(
                            &document, terminal, class, property, "#123456",
                        );
                    } else {
                        assert_eq!(
                            terminal.attribute("style"),
                            Some(
                                format!("{}:{color};", if html { "color" } else { "fill" })
                                    .as_str()
                            ),
                        );
                    }
                }
                if owned {
                    let baseline = render(&baseline_theme);
                    assert_eq!(
                        rendered.svg(),
                        baseline.svg(),
                        "source-owned labels must retain the complete source-only output",
                    );
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.applied_count(), if owned { 0 } else { 2 });
                assert_eq!(evidence.not_applicable_count(), if owned { 2 } else { 0 });
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn block_label_assigned_color_is_local_and_default_is_conditional() {
    let theme = block_cluster_theme([
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::ClusterLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
        ),
    ]);
    for html in [false, true] {
        for (suffix, class, native_owned, html_owned) in [
            (
                "classDef brand color:#123456\nclass group brand\n",
                "brand",
                [true, false, false],
                [false, false, false],
            ),
            (
                "classDef brand color:#123456\nclass A brand\n",
                "brand",
                [false, true, false],
                [false, false, false],
            ),
            (
                "classDef unused color:#123456\n",
                "unused",
                [false, false, false],
                [false, false, false],
            ),
            (
                "classDef default color:#123456\n",
                "default",
                [true, true, true],
                [false, false, false],
            ),
            (
                "classDef default color:#123456\nclassDef plain stroke:#654321\nclass group,A,B plain\n",
                "default",
                [false, false, false],
                [false, false, false],
            ),
            (
                "classDef brand fill:#123456\nclass group,A,B brand\n",
                "brand",
                [false, false, false],
                [false, false, false],
            ),
            (
                "classDef nodeLabel color:#123456\nclass group,A,B nodeLabel\n",
                "nodeLabel",
                [true, true, true],
                [true, true, true],
            ),
            (
                "classDef row color:#123456\nclass group,A,B row\n",
                "row",
                [true, true, true],
                [false, false, false],
            ),
        ] {
            let owned = if html { html_owned } else { native_owned };
            let source = format!("{BLOCK_CLUSTER_SOURCE}{suffix}");
            let rendered = render_block_with_theme_and_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"htmlLabels":html}),
                    ),
                ),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for ((id, color), owned) in [
                ("block-theme-group", "#2468ac"),
                ("block-theme-A", "#b316cd"),
                ("block-theme-B", "#b316cd"),
            ]
            .into_iter()
            .zip(owned)
            {
                let node = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .unwrap();
                let terminal = node
                    .descendants()
                    .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
                    .unwrap();
                assert_eq!(
                    terminal.attribute("style").unwrap_or("").contains(color),
                    !owned,
                    "{id}: html={html}, source={suffix}",
                );
                if owned {
                    assert_block_source_class_reaches_label_terminal(
                        &document, terminal, class, "color", "#123456",
                    );
                } else {
                    assert_eq!(
                        terminal.attribute("style"),
                        Some(format!("{}:{color};", if html { "color" } else { "fill" }).as_str()),
                    );
                }
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let applied = usize::from(!owned[0]) + usize::from(!owned[1] || !owned[2]);
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), applied);
            assert_eq!(evidence.not_applicable_count(), 2 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn block_node_label_ordinal_winner_keeps_an_unsupported_residual() {
    let rules = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
            )
            .for_family(DiagramFamilyId::BLOCK),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
            .with_ordinal(OrdinalSelector::exact(2).unwrap())
            .for_family(DiagramFamilyId::BLOCK),
        );
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(rules))
        .unwrap();
    let rendered = render_block_with_theme_requirement(
        "block\n A[\"Alpha\"] B[\"Beta\"]\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn block_explicit_default_font_size_has_a_typed_terminal_receipt() {
    for explicit in [false, true] {
        let style = if explicit {
            ThemeTextStyle::default().with_font_size_px(16.0).unwrap()
        } else {
            ThemeTextStyle::default()
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::BLOCK, style),
            ))
            .unwrap();
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\"Alpha\"]\n",
            &theme,
            MermaidEngine::new(),
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), usize::from(explicit));
        assert_eq!(evidence.applied_count(), usize::from(explicit));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn block_ignored_base_sizes_do_not_own_typed_typography() {
    let theme = block_typography_theme("Courier New", 29.0);
    let body = "block\nA[\"Alpha\"] --> B[\"Beta\"]\n";
    for value in [
        serde_json::json!(false),
        serde_json::json!("bad"),
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!([]),
        serde_json::json!(-2),
        serde_json::json!("-2"),
        serde_json::json!("-2px"),
        serde_json::json!(23),
        serde_json::json!("23px"),
    ] {
        let expected = if value.as_f64() == Some(23.0) || value.as_str() == Some("23px") {
            23
        } else {
            29
        };
        let config = serde_json::json!({"themeVariables":{"fontSize":value}});
        for from_source in [false, true] {
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config.clone())
            };
            let rendered = render_block_with_theme_and_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
            );
            assert!(
                rendered.svg().contains(&format!("font-size:{expected}px;")),
                "source={from_source}: {value}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1 + usize::from(expected == 29));
            assert_eq!(evidence.not_applicable_count(), usize::from(expected != 29));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn block_font_ownership_follows_accepted_strings_and_first_array_entries() {
    let theme = block_typography_theme("Courier New", 29.0);
    let body = "block\nA[\"Alpha\"]\n";
    for (value, expected) in [
        (serde_json::json!(42), "\"Courier New\""),
        (serde_json::json!(false), "\"Courier New\""),
        (serde_json::json!(""), "\"Courier New\""),
        (serde_json::json!([""]), "\"Courier New\""),
        (serde_json::json!([42, "Arial"]), "\"Courier New\""),
        (serde_json::json!(["Georgia"]), "Georgia"),
        (serde_json::json!("Georgia"), "Georgia"),
    ] {
        let config = serde_json::json!({"themeVariables":{"fontFamily":value}});
        for from_source in [false, true] {
            // Source initialization mirrors the root font over non-string theme variables.
            let expected = if from_source && value.is_array() {
                "\"Courier New\""
            } else {
                expected
            };
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config.clone())
            };
            let rendered = render_block_with_theme_and_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
            );
            assert!(
                rendered.svg().contains(&format!("font-family:{expected};")),
                "source={from_source}: {value}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(
                evidence.applied_count(),
                1 + usize::from(expected != "Georgia")
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(expected == "Georgia")
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn block_typed_typography_reaches_layout_css_and_strict_receipt() {
    let theme = block_typography_theme("Block Sans", 22.0);
    let rendered = render_block_with_theme_and_engine(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
        &theme,
        MermaidEngine::new(),
    );
    let svg = rendered.svg();
    assert!(
        svg.contains("#block-theme{font-family:\"Block Sans\";font-size:22px;"),
        "typed Block typography must reach the writer-owned root CSS: {svg}"
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_typography_receipt_matches_writer_number_formatting() {
    let theme = block_typography_theme("Block Sans", 1.0000001);
    let rendered = render_block_with_theme_and_engine(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
        &theme,
        MermaidEngine::new(),
    );
    assert!(
        rendered
            .svg()
            .contains("#block-theme{font-family:\"Block Sans\";font-size:1px;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_mixed_base_typography_fails_closed() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("Mixed Block Font").expect("valid Block font stack"))
        .with_font_size_px(22.0)
        .expect("valid Block font size")
        .with_font_weight(700)
        .expect("valid unsupported Block font weight");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::BLOCK, typography),
        ))
        .expect("compile mixed Block typography theme");
    let rendered = render_block_with_theme_requirement(
        "block\n  A[\"Alpha\"]\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn block_explicit_typography_ownership_outranks_typed_values() {
    let theme = block_typography_theme("Typed Block Sans", 22.0);
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "fontFamily": "Site Block Sans",
            "themeVariables": { "fontSize": "30px" }
        })),
    );
    let rendered = render_block_with_theme_and_engine(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
        &theme,
        engine,
    );
    let svg = rendered.svg();
    assert!(svg.contains("#block-theme{font-family:Site Block Sans;font-size:30px;"));
    assert!(!svg.contains("Typed Block Sans"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
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

fn terminal_node_has_class(svg: &str, node_id: &str, expected_class: &str) -> bool {
    let document = roxmltree::Document::parse(svg).expect("valid Block SVG");
    document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .and_then(|node| node.attribute("class"))
        .is_some_and(|classes| {
            classes
                .split_ascii_whitespace()
                .any(|class| class == expected_class)
        })
}

#[test]
fn block_link_tokens_reach_svg_markers_and_line_classes() {
    for (token, start, end, thickness, pattern) in [
        ("-->", false, true, "normal", "solid"),
        ("<-->", true, true, "normal", "solid"),
        ("<==>", true, true, "thick", "solid"),
        ("==>", false, true, "thick", "solid"),
        ("-.->", false, true, "normal", "dotted"),
        ("===", false, false, "thick", "solid"),
        ("-.-", false, false, "normal", "dotted"),
    ] {
        let svg = render_block_svg_from_text(&format!("block\nA{token}B\n"));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let path = document
            .descendants()
            .find(|node| node.attribute("data-edge") == Some("true"))
            .unwrap();
        let classes = path
            .attribute("class")
            .unwrap()
            .split_ascii_whitespace()
            .collect::<Vec<_>>();
        assert!(
            classes.contains(&format!("edge-thickness-{thickness}").as_str()),
            "{token}"
        );
        assert!(
            classes.contains(&format!("edge-pattern-{pattern}").as_str()),
            "{token}"
        );
        for (attribute, expected) in [("marker-start", start), ("marker-end", end)] {
            let reference = path.attribute(attribute);
            assert_eq!(reference.is_some(), expected, "{token} {attribute}");
            if let Some(reference) = reference {
                let id = reference
                    .strip_prefix("url(#")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                assert!(document.descendants().any(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id)));
            }
        }
    }
}

#[test]
fn block_spaced_class_targets_keep_attached_node_identity() {
    let svg = render_block_svg_from_text("block\nA[\"Alpha\"] B[\"Beta\"]\nclass A, B hot\n");
    let document = roxmltree::Document::parse(&svg).unwrap();
    let nodes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "node")
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 2);
    let classes = |node: roxmltree::Node<'_, '_>| {
        node.attribute("class")
            .unwrap()
            .split_ascii_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert!(classes(nodes[0]).iter().any(|class| class == "hot"));
    assert!(!classes(nodes[0]).iter().any(|class| class == "B"));
    assert!(!classes(nodes[1]).iter().any(|class| class == "hot"));
    assert!(
        nodes[1]
            .descendants()
            .any(|node| node.text() == Some("Beta"))
    );
}

#[test]
fn block_detached_database_records_do_not_require_visible_html_labels() {
    for source in ["block\nA space B\n", "block\nA\nclass missing hot\n"] {
        let svg = render_block_svg_from_text(source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(document.descendants().any(|node| node.text() == Some("A")));
    }
}

fn terminal_stroke(style: &str) -> Option<&str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case("stroke"))
        .map(|(_, value)| value.trim())
        .next_back()
}

fn terminal_fill(style: &str) -> Option<&str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case("fill"))
        .map(|(_, value)| value.trim())
        .next_back()
}

#[test]
fn block_typed_node_fill_reaches_every_terminal_shape() {
    let source = r#"block-beta
  columns 5
  rect["Rect"] circle(("Circle")) double((("Double"))) cylinder[("Cylinder")] polygon{{"Polygon"}}
"#;
    let cases = [
        (
            CanvasPaint::solid("#654321").expect("valid Block node fill"),
            "#654321",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (fill, expected_fill) in cases {
        let theme = block_node_fill_theme(fill);
        let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());
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
                    .all(|(_, style)| terminal_fill(style) == Some(expected_fill)),
                "typed fill must reach every terminal shell for {node_id}: {shells:?}"
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
fn block_ordinal_palette_is_not_applicable_when_typed_fill_wins() {
    let theme = block_node_fill_with_ordinal_palette_theme(
        CanvasPaint::solid("#654321").expect("valid typed Block node fill"),
    );
    let rendered =
        render_block_with_theme_and_engine("block\n  A[\"Alpha\"]\n", &theme, MermaidEngine::new());
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_ordinal_palette_is_not_applicable_when_source_fill_wins() {
    let source = r#"block
  A["Alpha"]
  classDef default fill:#bb0000
"#;
    let rendered = render_block_with_theme_and_engine(
        source,
        &block_node_ordinal_palette_theme(),
        MermaidEngine::new(),
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_typed_node_fill_and_stroke_share_the_same_shell_truth() {
    let theme = block_node_fill_and_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid Block node fill"),
        CanvasPaint::solid("#123456").expect("valid Block node stroke"),
    );
    let rendered = render_block_with_theme_and_engine(
        "block-beta\n  double(((\"Double\")))\n",
        &theme,
        MermaidEngine::new(),
    );
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-double");

    assert_eq!(shells.len(), 2, "expected both circle shells: {shells:?}");
    assert!(
        shells.iter().all(|(_, style)| {
            terminal_fill(style) == Some("#654321") && terminal_stroke(style) == Some("#123456")
        }),
        "fill and stroke must be proved on every shell: {shells:?}"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_mixed_node_paint_rule_reconciles_source_ownership_per_property_and_node() {
    let source = r#"block-beta
  columns 2
  fillOwned["Fill owned"] strokeOwned(("Stroke owned"))
  style fillOwned fill:#0000bb
  style strokeOwned stroke:#00bb00
"#;
    let theme = block_node_fill_and_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid Block node fill"),
        CanvasPaint::solid("#123456").expect("valid Block node stroke"),
    );
    let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());

    let fill_owned = terminal_shell_styles(rendered.svg(), "block-theme-fillOwned");
    assert_eq!(terminal_fill(&fill_owned[0].1), Some("#0000bb"));
    assert_eq!(terminal_stroke(&fill_owned[0].1), Some("#123456"));

    let stroke_owned = terminal_shell_styles(rendered.svg(), "block-theme-strokeOwned");
    assert_eq!(terminal_fill(&stroke_owned[0].1), Some("#654321"));
    assert_eq!(terminal_stroke(&stroke_owned[0].1), Some("#00bb00"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_source_owned_fill_does_not_hide_direct_stroke_evidence() {
    let source = r#"block-beta
  columns 2
  first["First"] second(("Second"))
  classDef default fill:#bb0000
"#;
    let theme = block_node_fill_and_stroke_theme(
        CanvasPaint::Transparent,
        CanvasPaint::solid("#123456").expect("valid Block node stroke"),
    );
    let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());

    for node_id in ["block-theme-first", "block-theme-second"] {
        assert!(
            terminal_node_has_class(rendered.svg(), node_id, "default"),
            "source-owned default class must be attached to {node_id}: {}",
            rendered.svg()
        );
        let shells = terminal_shell_styles(rendered.svg(), node_id);
        assert!(
            shells.iter().all(|(_, style)| {
                terminal_fill(style) != Some("transparent")
                    && terminal_stroke(style) == Some("#123456")
            }),
            "source fill and typed stroke must remain property-local for {node_id}: {shells:?}"
        );
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_inline_and_class_fills_outrank_typed_node_fill_per_node() {
    let source = r#"block-beta
  columns 3
  bare["Bare"] assigned(("Assigned")) inline{{"Inline"}}
  classDef accent fill:#00bb00
  class assigned accent
  style inline fill:#0000bb
"#;
    let theme =
        block_node_fill_theme(CanvasPaint::solid("#654321").expect("valid Block node fill"));
    let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());

    let bare = terminal_shell_styles(rendered.svg(), "block-theme-bare");
    assert_eq!(terminal_fill(&bare[0].1), Some("#654321"), "{bare:?}");
    let inline = terminal_shell_styles(rendered.svg(), "block-theme-inline");
    assert_eq!(terminal_fill(&inline[0].1), Some("#0000bb"), "{inline:?}");
    let assigned = terminal_shell_styles(rendered.svg(), "block-theme-assigned");
    assert!(
        assigned
            .iter()
            .all(|(_, style)| terminal_fill(style) != Some("#654321")),
        "assigned class must suppress typed inline fill: {assigned:?}"
    );
    assert!(
        terminal_node_has_class(rendered.svg(), "block-theme-assigned", "accent"),
        "the source-owned class must be attached to the assigned node: {}",
        rendered.svg()
    );
    assert!(
        block_stylesheet_text(rendered.svg())
            .contains(r#"#block-theme .accent>*{fill:#00bb00!important;}"#),
        "assigned class fill must retain source ownership: {}",
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
fn block_default_class_fill_makes_typed_node_fill_not_applicable() {
    let source = r#"block
  A["Alpha"]
  classDef default fill:#bb0000
"#;
    let theme = block_node_fill_theme(
        CanvasPaint::solid("#654321").expect("valid suppressed Block node fill"),
    );
    let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_fill(style) != Some("#654321")),
        "default class must suppress typed inline fill: {shells:?}"
    );
    assert!(
        terminal_node_has_class(rendered.svg(), "block-theme-A", "default"),
        "the implicit default class must be attached to the node: {}",
        rendered.svg()
    );
    assert!(
        block_stylesheet_text(rendered.svg())
            .contains(r#"#block-theme .default>*{fill:#bb0000!important;}"#),
        "default class fill must retain source ownership: {}",
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
fn block_explicit_main_background_outranks_typed_node_fill() {
    let theme = block_node_fill_theme(
        CanvasPaint::solid("#654321").expect("valid config-suppressed Block node fill"),
    );
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "mainBkg": "#445566" }
        })),
    );
    let rendered = render_block_with_theme_and_engine("block\n  A[\"Alpha\"]\n", &theme, engine);
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_fill(style) != Some("#654321")),
        "explicit mainBkg must suppress typed inline fill: {shells:?}"
    );
    assert!(
        rendered.svg().contains("fill:#445566;stroke:"),
        "explicit mainBkg must remain the Block base fill: {}",
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
fn block_explicit_default_fill_uses_the_typed_writer() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#654321")
                                .expect("valid legacy-compatible Block node fill"),
                        ),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile explicit Default Block node fill");
    let rendered = render_block_with_theme_requirement(
        "block\n  A[\"Alpha\"]\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .any(|(_, style)| terminal_fill(style) == Some("#654321")),
        "explicit Default must use the typed writer: {shells:?}"
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
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
        let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());
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
    let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());

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
        terminal_node_has_class(rendered.svg(), "block-theme-assigned", "accent"),
        "the source-owned class must be attached to the assigned node: {}",
        rendered.svg()
    );
    assert!(
        block_stylesheet_text(rendered.svg())
            .contains(r#"#block-theme .accent>*{stroke:#00bb00!important;}"#),
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
    let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#123456")),
        "default class must suppress typed inline stroke: {shells:?}"
    );
    assert!(
        terminal_node_has_class(rendered.svg(), "block-theme-A", "default"),
        "the implicit default class must be attached to the node: {}",
        rendered.svg()
    );
    assert!(
        block_stylesheet_text(rendered.svg())
            .contains(r#"#block-theme .default>*{stroke:#bb0000!important;}"#),
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "nodeBorder": "#445566" }
        })),
    );
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
        &MermaidEngine::new(),
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
    let engine = MermaidEngine::new();
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
        svg.contains(r#"#merman .edgePaths .path{stroke:#112233;stroke-width:2.0px;}"#),
        "expected Block edge-path CSS to use Mermaid's edgePaths contract: {svg}"
    );
    assert!(
        svg.contains(r#"class="edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1""#),
        "expected Block edge path to carry the themed edge-thickness-normal class: {svg}"
    );
}

#[test]
fn block_node_stroke_width_matches_its_terminal_css_for_classic_and_neo() {
    for look in ["classic", "neo"] {
        for width in [0, 1, 4] {
            let (svg, _) = render_block_geometry(
                serde_json::json!({"look": look, "themeVariables": {"strokeWidth": width}}),
                "block\n A[\"A\"]\n",
            );
            let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
            assert_eq!(
                matched_block_node_stroke_width(&document, "merman-A"),
                format!("{width}px"),
                "{look} node terminal must consume the configured stroke width"
            );
        }
    }
}

#[test]
fn block_root_viewbox_contains_configured_node_stroke_envelope() {
    for width in [0.0, 4.0, 24.0] {
        let (svg, _) = render_block_geometry(
            serde_json::json!({"themeVariables": {"strokeWidth": width}}),
            "block\n A[\"A\"]\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
        let rect = block_node_rect(&document, "merman-A");
        let (center_x, center_y) = translated_center(rect.parent().unwrap());
        let min_x = center_x + numeric_attribute(rect, "x");
        let min_y = center_y + numeric_attribute(rect, "y");
        let max_x = min_x + numeric_attribute(rect, "width");
        let max_y = min_y + numeric_attribute(rect, "height");
        let (view_x, view_y, view_width, view_height) = root_view_box(&document);
        let margin = width / 2.0 + 5.0;
        assert!(
            view_x <= min_x - margin + 1e-6
                && view_y <= min_y - margin + 1e-6
                && view_x + view_width >= max_x + margin - 1e-6
                && view_y + view_height >= max_y + margin - 1e-6,
            "stroke {width} envelope plus Mermaid viewport padding must fit: {svg}"
        );
    }
}

#[test]
fn block_square_padding_uses_look_while_grid_spacing_uses_block_padding() {
    for look in ["classic", "neo"] {
        for padding in [8.0, 20.0] {
            for source in [
                "block\n A[\"A\"]\n",
                "block\n columns 2\n A[\"A\"] B[\"Longer\"]\n",
            ] {
                let (svg, layout) = render_block_geometry(
                    serde_json::json!({"look": look, "block": {"padding": padding}}),
                    source,
                );
                let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
                let nodes = layout["nodes"].as_array().expect("layout nodes");
                let max_label_width = nodes
                    .iter()
                    .map(|node| {
                        let id = node["id"].as_str().unwrap();
                        let rect = block_node_rect(&document, &format!("merman-{id}"));
                        let label = rect
                            .parent()
                            .unwrap()
                            .descendants()
                            .find(|node| node.has_tag_name("foreignObject"))
                            .expect("Block HTML label");
                        numeric_attribute(label, "width")
                    })
                    .fold(0.0, f64::max);
                for node in nodes {
                    let id = node["id"].as_str().unwrap();
                    let rect = block_node_rect(&document, &format!("merman-{id}"));
                    let geometry = block_shape_geometry(&layout, id);
                    let shell_x_padding = if look == "neo" { 32.0 } else { 4.0 * padding };
                    let shell_y_padding = if look == "neo" { 24.0 } else { 2.0 * padding };
                    let expected_width = max_label_width + shell_x_padding;
                    let label = rect
                        .parent()
                        .unwrap()
                        .descendants()
                        .find(|node| node.has_tag_name("foreignObject"))
                        .expect("Block HTML label");
                    let expected_height = numeric_attribute(label, "height") + shell_y_padding;
                    assert!((numeric_attribute(rect, "width") - expected_width).abs() <= 1e-6);
                    assert!((numeric_attribute(rect, "height") - expected_height).abs() <= 1e-6);
                    assert_eq!(geometry["boundary"]["width"].as_f64(), Some(expected_width));
                    assert_eq!(
                        geometry["boundary"]["height"].as_f64(),
                        Some(expected_height)
                    );
                    assert_eq!(
                        geometry["allocated"]["width"].as_f64(),
                        Some(expected_width)
                    );
                    assert_eq!(
                        geometry["allocated"]["height"].as_f64(),
                        Some(expected_height)
                    );
                }
                if nodes.len() == 2 {
                    let a = block_node_rect(&document, "merman-A");
                    let b = block_node_rect(&document, "merman-B");
                    let (ax, _) = translated_center(a.parent().unwrap());
                    let (bx, _) = translated_center(b.parent().unwrap());
                    let gap = bx
                        - ax
                        - numeric_attribute(a, "width") / 2.0
                        - numeric_attribute(b, "width") / 2.0;
                    assert!((gap - padding).abs() <= 1e-6, "{look} grid gap: {gap}");
                }
            }
        }
    }
}

#[test]
fn block_round_radius_preserves_mermaid_truthiness_in_svg_and_geometry() {
    for (radius, expected) in [
        (None, 5.0),
        (Some(serde_json::json!(12)), 12.0),
        (Some(serde_json::json!(0)), 5.0),
        (Some(serde_json::json!("0")), 0.0),
        (Some(serde_json::Value::Null), 5.0),
    ] {
        let mut config = serde_json::json!({});
        if let Some(radius) = radius {
            config["themeVariables"] = serde_json::json!({"radius": radius});
        }
        let (svg, layout) = render_block_geometry(config, "block\n A(\"A\")\n");
        let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
        let rect = block_node_rect(&document, "merman-A");
        assert_eq!(numeric_attribute(rect, "rx"), expected);
        assert_eq!(numeric_attribute(rect, "ry"), expected);
        assert_eq!(
            block_shape_geometry(&layout, "A")["boundary"]["radius"].as_f64(),
            Some(expected)
        );
    }
}

#[test]
fn block_round_diagonal_edge_uses_rectangular_routing_with_any_paint_radius() {
    for radius in [5, 12] {
        let (svg, layout) = render_block_geometry(
            serde_json::json!({"themeVariables": {"radius": radius}}),
            "block-beta\n columns 2\n A(\"A\") B[\"B\"]\n space C[\"C\"]\n A --- C\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
        let a = block_node_rect(&document, "merman-A");
        let layout_edge = layout["edges"]
            .as_array()
            .expect("Block layout edges")
            .iter()
            .find(|edge| edge["from"] == "A" && edge["to"] == "C")
            .expect("A to C semantic edge");
        let data_id = format!(
            "{}-{}",
            document.root_element().attribute("id").expect("diagram id"),
            layout_edge["id"].as_str().expect("semantic edge id")
        );
        let edge = document
            .descendants()
            .find(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-et") == Some("edge")
                    && node.attribute("data-id") == Some(data_id.as_str())
            })
            .expect("markerless A to C edge");
        assert!(edge.attribute("marker-start").is_none());
        assert!(edge.attribute("marker-end").is_none());
        let points = base64::engine::general_purpose::STANDARD
            .decode(edge.attribute("data-points").expect("raw edge points"))
            .expect("base64 raw edge points");
        let points: Vec<merman_render::model::LayoutPoint> =
            serde_json::from_slice(&points).expect("raw edge point JSON");
        let (center_x, center_y) = translated_center(a.parent().unwrap());
        let dx = points[0].x - center_x;
        let dy = points[0].y - center_y;
        let half_width = numeric_attribute(a, "width") / 2.0;
        let half_height = numeric_attribute(a, "height") / 2.0;
        assert!(dx.abs() <= half_width + 1e-9 && dy.abs() <= half_height + 1e-9);
        assert!(
            (dx.abs() - half_width).abs() <= 1e-9 || (dy.abs() - half_height).abs() <= 1e-9,
            "radius {radius} must route to the rectangle: endpoint=({dx},{dy}), half size=({half_width},{half_height})"
        );
        assert_eq!(
            block_shape_geometry(&layout, "A")["boundary"]["radius"].as_f64(),
            Some(f64::from(radius))
        );
    }
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
    for variables in [
        serde_json::json!({"nodeTextColor": "#123456", "textColor": "#abcdef"}),
        serde_json::json!({"textColor": "#123456"}),
        serde_json::json!({"nodeTextColor": "", "textColor": "#123456"}),
        serde_json::json!({"nodeTextColor": null, "textColor": "#123456"}),
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"themeVariables": variables}),
            ),
        );
        let svg = render_block_svg_from_text_with_engine(
            &engine,
            r#"block
  A["Alpha"]
"#,
        );

        assert!(
            svg.contains(
                r#"#merman .label text,#merman span,#merman p{fill:#123456;color:#123456;}"#
            ),
            "expected nodeTextColor || textColor to drive Block label color: {variables}"
        );
    }
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {
                "clusterBkg": "not-a-css-color"
            }
        })),
    );
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
        svg.contains(r#"class="node front flowchart-label""#),
        "expected the Frontend node to retain its assigned class: {svg}"
    );
    let document = roxmltree::Document::parse(&svg).expect("Block class stylesheet XML");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Block class stylesheet");
    assert!(
        css.contains(r#"#merman .front>*{fill:#696!important;stroke:#333!important;}"#),
        "expected the front class definition to style Block shapes: {svg}"
    );
    assert!(
        css.contains(r#"#merman .back>*{fill:#969!important;stroke:#333!important;}"#),
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
    let document = roxmltree::Document::parse(&svg).expect("classDef CSS XML");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("classDef CSS");
    assert!(
        css.contains(r#"font-family:"A&B"!important;"#),
        "classDef values must be logical CSS after XML parsing: {css}"
    );
}

#[test]
fn block_svg_drops_unsafe_inline_css_without_losing_safe_siblings() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"]
  style A fill:url(https://example.test/paint.svg),stroke:#123456
"#,
    );

    assert!(
        !svg.contains("example.test") && !svg.contains("url(https://"),
        "unsafe external CSS resources must not reach the SVG: {svg}"
    );
    let shells = terminal_shell_styles(&svg, "merman-A");
    assert_eq!(shells.len(), 1, "expected one canonical Block shell");
    assert_eq!(terminal_stroke(&shells[0].1), Some("#123456"));
    assert_eq!(terminal_fill(&shells[0].1), None);
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
fn block_root_viewbox_uses_rendered_shape_bounds() {
    let svg = render_block_svg_from_text(
        r#"block
  id1((("This is the text in the circle")))
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-id1"))
        .expect("rendered double-circle node");
    let outer_circle = node
        .descendants()
        .find(|node| node.attribute("class") == Some("outer-circle"))
        .expect("rendered outer circle");
    let (center_x, center_y) = translated_center(node);
    let radius = outer_circle
        .attribute("r")
        .expect("outer circle radius")
        .parse::<f64>()
        .expect("numeric outer circle radius");
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let padding = 5.0;

    assert!(view_x <= center_x - radius - padding + 1e-9);
    assert!(view_y <= center_y - radius - padding + 1e-9);
    assert!(view_x + view_width >= center_x + radius + padding - 1e-9);
    assert!(view_y + view_height >= center_y + radius + padding - 1e-9);
}

#[test]
fn block_root_viewbox_ignores_non_schema_diagram_padding() {
    let default_svg = render_block_svg_from_text(
        r#"block
  A["A"] --> B["B"]
"#,
    );
    let opaque_config_svg = render_block_svg_from_text(
        r#"%%{init: {"block": {"diagramPadding": 250}}}%%
block
  A["A"] --> B["B"]
"#,
    );
    let default_document = roxmltree::Document::parse(&default_svg).expect("default Block SVG");
    let opaque_config_document =
        roxmltree::Document::parse(&opaque_config_svg).expect("configured Block SVG");

    assert_eq!(
        root_view_box(&default_document),
        root_view_box(&opaque_config_document),
        "Block viewBox padding is the fixed Mermaid 5px expansion, not an opaque config surface"
    );
}

#[test]
fn block_root_viewbox_contains_edge_label_foreign_object() {
    let svg = render_block_svg_from_text(
        r#"block
  A["A"] -- "A very long edge label" --> B["B"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let edge_label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("edgeLabel")
                && node
                    .descendants()
                    .any(|descendant| descendant.has_tag_name("foreignObject"))
        })
        .expect("rendered Block edge label group");
    let foreign_object = edge_label
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .expect("rendered Block edge label foreignObject");
    let (label_min_x, label_min_y, label_max_x, label_max_y) =
        global_foreign_object_bounds(foreign_object);
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let view_max_x = view_x + view_width;
    let view_max_y = view_y + view_height;

    assert!(
        label_min_x >= view_x - 1e-6
            && label_min_y >= view_y - 1e-6
            && label_max_x <= view_max_x + 1e-6
            && label_max_y <= view_max_y + 1e-6,
        "edge label foreignObject must be contained by the root viewBox: label=({label_min_x},{label_min_y})..({label_max_x},{label_max_y}), viewBox=({view_x},{view_y}) {view_width}x{view_height}, svg={svg}"
    );
}

#[test]
fn block_html_labels_false_emits_svg_text_for_nodes_and_edges() {
    let svg = render_block_svg_from_text(
        r#"%%{init: {"htmlLabels": false}}%%
block
  A["Alpha"] -- "Edge label" --> B["Beta"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");

    assert!(
        document
            .descendants()
            .all(|node| !node.has_tag_name("foreignObject")),
        "htmlLabels:false must not emit foreignObject labels: {svg}"
    );

    let text_content = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .flat_map(|node| {
            node.descendants()
                .filter_map(|descendant| descendant.text())
        })
        .collect::<String>();
    assert!(
        text_content.contains("Alpha")
            && text_content.contains("Beta")
            && text_content.contains("Edge label"),
        "htmlLabels:false must preserve node and edge label text in SVG text terminals: {svg}"
    );

    for label in document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
    {
        let (x, _) = translated_center(label);
        assert_eq!(x, 0.0, "SVG text is already horizontally centered: {svg}");
    }
    let background = document
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
        .expect("edge label background");
    for attribute in ["width", "height"] {
        assert!(
            background
                .attribute(attribute)
                .unwrap()
                .parse::<f64>()
                .unwrap()
                > 0.0
        );
    }
}

#[test]
fn block_svg_labels_preserve_inline_text_color() {
    let svg = render_block_svg_from_text(
        r#"%%{init: {"htmlLabels": false}}%%
block
  A["Alpha"] B["Beta"] C["Variable"]
  style A fill:#eee,color:#123456,color:#abcdef
  style B color:hsl(80 100% 96%)
  style C color:var(--MyColor)
"#,
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (id, expected) in [
        ("merman-A", "fill:#abcdef;"),
        ("merman-B", "fill:hsl(80 100% 96%);"),
        ("merman-C", "fill:var(--MyColor);"),
    ] {
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap();
        let text = node
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(text.attribute("style"), Some(expected), "{id}: {svg}");
    }
}

#[test]
fn block_svg_edge_background_uses_operation_text_bounds() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::TextMetrics;
    use std::sync::Arc;

    struct LabelHost(f64);
    impl HostTextMeasurer for LabelHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(match request.operation {
                TextMeasurementOperation::Wrapped => {
                    Some(HostTextMeasurement::Metrics(TextMetrics {
                        width: 80.0,
                        height: 40.0,
                        line_count: 2,
                    }))
                }
                TextMeasurementOperation::CreateTextBBoxYOffset => {
                    assert_eq!(request.text, "First\nSecond");
                    assert_eq!(request.style.font_size, 16.0);
                    Some(HostTextMeasurement::Length(self.0))
                }
                _ => None,
            })
        }
    }
    for bbox_y in [-7.5, 9.0] {
        let parsed = MermaidEngine::new().parse_diagram_for_render_model_sync(
            "%%{init: {\"htmlLabels\": false}}%%\nblock\n A[\"Alpha<br/>line\"] -- \"First<br/>Second\" --> B[\"Beta\"]\n",
            ParseOptions::default(),
        ).unwrap().unwrap().into_presentation();
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.block-svg-label").unwrap(),
            "1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                Arc::new(LabelHost(bbox_y)),
                TextMeasurementPhase::ALL,
            ))
            .begin_session()
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rect = document
            .descendants()
            .find(|node| node.attribute("class") == Some("background"))
            .unwrap();
        let number = |name| rect.attribute(name).unwrap().parse::<f64>().unwrap();
        assert_eq!((number("width"), number("height")), (84.0, 44.0));
        assert_eq!((number("x"), number("y")), (-42.0, bbox_y - 2.0));
        let label = rect.parent().unwrap();
        let (tx, ty) = translated_center(label);
        assert_eq!((tx, ty), (0.0, -bbox_y - 20.0));
        assert_eq!(tx + number("x") + number("width") / 2.0, 0.0);
        assert_eq!(ty + number("y") + number("height") / 2.0, 0.0);
        let (edge_x, edge_y) = translated_center(label.parent().unwrap());
        let (view_x, view_y, view_w, view_h) = root_view_box(&document);
        assert!(edge_x - 42.0 >= view_x && edge_x + 42.0 <= view_x + view_w);
        assert!(edge_y - 22.0 >= view_y && edge_y + 22.0 <= view_y + view_h);
        let rows = label
            .descendants()
            .filter(|node| node.attribute("class") == Some("row text-outer-tspan"))
            .count();
        assert_eq!(rows, 2);
    }
}

#[test]
fn block_root_viewbox_contains_rough_stroke_bounds() {
    let svg = render_block_svg_from_text(
        r#"block
  A(["A"])
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let stadium = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-A"))
        .expect("rendered stadium node");
    let outer_path = stadium
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|class| class == "basic label-container outer-path")
        })
        .expect("rendered stadium outer-path group");
    let stroke_path = outer_path
        .children()
        .find(|node| node.has_tag_name("path") && node.attribute("fill") == Some("none"))
        .expect("RoughJS stadium stroke path");
    let (min_x, min_y, max_x, max_y) = global_path_geometry_bounds(stroke_path);
    let stroke_width = stroke_path
        .attribute("stroke-width")
        .expect("RoughJS stroke width")
        .parse::<f64>()
        .expect("numeric RoughJS stroke width");
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let view_max_x = view_x + view_width;
    let view_max_y = view_y + view_height;
    let outset = stroke_width / 2.0;

    assert!(
        min_x - outset >= view_x - 1e-6
            && min_y - outset >= view_y - 1e-6
            && max_x + outset <= view_max_x + 1e-6
            && max_y + outset <= view_max_y + 1e-6,
        "root viewBox must contain the emitted RoughJS stroke envelope: path=({min_x},{min_y})..({max_x},{max_y}), stroke={stroke_width}, viewBox=({view_x},{view_y}) {view_width}x{view_height}, svg={svg}"
    );
}

#[test]
fn block_stadium_renders_rough_outer_path_and_clips_edge() {
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
    let outer_path = stadium
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|class| class == "basic label-container outer-path")
        })
        .expect("rendered stadium outer-path group");
    let paths = outer_path
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        2,
        "RoughJS stadiums emit fill and stroke paths"
    );
    assert_eq!(paths[0].attribute("stroke"), Some("none"));
    assert_eq!(paths[1].attribute("fill"), Some("none"));
    let stroke_width: f64 = paths[1]
        .attribute("stroke-width")
        .expect("RoughJS stroke width")
        .parse()
        .expect("numeric RoughJS stroke width");
    assert!((stroke_width - 1.3).abs() <= 1e-6);
    assert!(!outer_path.children().any(|node| node.has_tag_name("rect")));
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path") && node.attribute("id").is_some_and(|id| id.contains("A-B"))
        })
        .expect("stadium edge");

    let (center_x, center_y) = translated_center(stadium);
    let (edge_x, edge_y) = path_start(edge);
    assert!(
        edge_x > center_x,
        "edge must leave the stadium on its right side"
    );
    assert!((edge_y - center_y).abs() <= 1e-3);
}

#[test]
fn block_absent_text_rules_exclude_metadata_and_noncomposite_labels() {
    for target in [ThemeTarget::Title, ThemeTarget::ClusterLabel] {
        let source = if target == ThemeTarget::Title {
            "---\ntitle: Metadata title\n---\nblock\n  columns 1\n  block:group[\"Composite title\"]\n    A[\"Alpha\"]\n  end\n"
        } else {
            "---\ntitle: Metadata title\n---\nblock\n A[\"Alpha\"]\n"
        };
        for html_labels in [false, true] {
            for variant in [
                None,
                Some(ThemeVariant::Default),
                Some(ThemeVariant::Active),
            ] {
                for transparent in [false, true] {
                    let paint = if transparent {
                        CanvasPaint::Transparent
                    } else {
                        CanvasPaint::solid("#b316cd").unwrap()
                    };
                    let mut rule =
                        ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint))
                            .for_family(DiagramFamilyId::BLOCK);
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let theme = DiagramThemeCompiler::new()
                        .compile(
                            DiagramThemeSpec::new()
                                .with_styles(ThemeRuleSet::default().with_rule(rule)),
                        )
                        .unwrap();
                    let rendered = render_block_with_theme_and_engine(
                        source,
                        &theme,
                        MermaidEngine::new().with_site_config(
                            merman_core::presentation::SourcePresentationConfig::from_value(
                                serde_json::json!({
                                    "htmlLabels": html_labels
                                }),
                            ),
                        ),
                    );
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    assert!(document.descendants().all(|node| {
                        !node
                            .attribute("class")
                            .unwrap_or("")
                            .split_ascii_whitespace()
                            .any(|class| matches!(class, "flowchartTitleText" | "cluster-label"))
                    }));
                    for cluster in document.descendants().filter(|node| {
                        node.attribute("class")
                            .unwrap_or("")
                            .split_ascii_whitespace()
                            .any(|class| class == "cluster")
                    }) {
                        assert!(cluster.descendants().all(|node| {
                            !matches!(node.tag_name().name(), "text" | "span" | "p")
                        }));
                    }
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(evidence.required_count(), 1);
                    assert_eq!(evidence.applied_count(), 0);
                    assert_eq!(evidence.not_applicable_count(), 1);
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn absent_block_text_siblings_do_not_suppress_typed_node_paint() {
    use merman_render::diagram_theme::{OrdinalSelector, Specified};
    for target in [ThemeTarget::Title, ThemeTarget::ClusterLabel] {
        for clear in [false, true] {
            let mut patch = ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                .with_stroke(CanvasPaint::solid("#2468ac").unwrap());
            if clear {
                patch.paint.fill = Specified::Clear;
            }
            let absent_text = ThemeRule::new(target, patch)
                .for_family(DiagramFamilyId::BLOCK)
                .with_ordinal(OrdinalSelector::exact(1).unwrap());
            let node = ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#654321").unwrap()),
            )
            .for_family(DiagramFamilyId::BLOCK);
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default()
                            .with_rule(absent_text)
                            .with_rule(node),
                    ),
                )
                .unwrap();
            let rendered = render_block_with_theme_and_engine(
                "block\n A[\"Alpha\"]\n",
                &theme,
                MermaidEngine::new(),
            );
            assert!(
                terminal_shell_styles(rendered.svg(), "block-theme-A")
                    .iter()
                    .any(|(_, style)| terminal_fill(style) == Some("#654321"))
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn block_edge_label_background_typed_route_has_a_real_css_consumer() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::EdgeLabelBackground,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#b316cd").expect("valid paint")),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile Block edge-label theme");
    let rendered = render_block_with_theme_requirement(
        "block\n  A[\"Alpha\"] -- \"relates\" --> B[\"Beta\"]\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    assert!(
        rendered
            .svg()
            .contains("#block-theme .edgeLabel{background-color:#b316cd;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_background_reconciles_missing_labels_and_explicit_config() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::EdgeLabelBackground,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#b316cd").unwrap()),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .unwrap();
    for html_labels in [false, true] {
        for (case, background, config_owned) in [
            ("absent", None, false),
            ("string", Some(serde_json::json!("#123456")), true),
            ("false", Some(serde_json::json!(false)), false),
            ("null", Some(serde_json::Value::Null), false),
        ] {
            for has_label in [false, true] {
                let mut config = serde_json::json!({
                    "theme": "default", "look": "classic", "htmlLabels": html_labels
                });
                if let Some(background) = &background {
                    config["themeVariables"] =
                        serde_json::json!({"edgeLabelBackground": background});
                }
                let source = if has_label {
                    "block\n A -- \"relates\" --> B\n"
                } else {
                    "block\n A --> B\n"
                };
                let rendered = render_block_with_theme_and_engine(
                    source,
                    &theme,
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                );
                if let Some(background) = &background {
                    let config = &rendered.metadata().effective_config;
                    if background.is_null() {
                        assert_eq!(
                            config.get_str("themeVariables.edgeLabelBackground"),
                            Some("rgba(232,232,232, 0.8)"),
                            "Default materializes the null input as its fallback paint"
                        );
                    } else {
                        assert_eq!(
                            &config.as_value()["themeVariables"]["edgeLabelBackground"],
                            background,
                            "{case}: check the actual materialized CSS input before its owner"
                        );
                    }
                }
                let expected = if config_owned { "#123456" } else { "#b316cd" };
                assert!(
                    rendered.svg().contains(&format!(
                        "#block-theme .edgeLabel{{background-color:{expected};"
                    )),
                    "{case}, html_labels={html_labels}, has_label={has_label}"
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                let applied = usize::from(has_label && !config_owned);
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.accounted_count(), 1);
                assert_eq!(evidence.applied_count(), applied, "{case}");
                assert_eq!(evidence.not_applicable_count(), 1 - applied, "{case}");
                assert_eq!(evidence.theme_residual_count(), 0, "{case}");
            }
        }
    }
}

#[test]
fn block_background_unsupported_winning_facet_stays_residual() {
    for ordinal in [false, true] {
        let patch = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap());
        let patch = if ordinal {
            patch
        } else {
            patch.with_stroke(CanvasPaint::solid("#2468ac").unwrap())
        };
        let rule = ThemeRule::new(ThemeTarget::EdgeLabelBackground, patch)
            .for_family(DiagramFamilyId::BLOCK);
        let rule = if ordinal {
            rule.with_ordinal(OrdinalSelector::exact(2).unwrap())
        } else {
            rule
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();
        let source = "block\n A -- \"first\" --> B\n B -- \"second\" --> C\n";
        let rendered = render_block_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap();
        let result = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
        assert!(
            matches!(
                result,
                Err(merman_render::Error::UnverifiedFamilyTheme {
                    family_id: DiagramFamilyId::BLOCK,
                    residual_count: 1,
                })
            ),
            "unsupported winning background facet must produce a Block theme residual"
        );
    }
}

fn block_edge_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let rules = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), |set, rule| {
            set.with_rule(rule.for_family(DiagramFamilyId::BLOCK))
        });
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(rules))
        .unwrap()
}

#[test]
fn block_edge_static_paint_is_written_only_to_visible_edge_paths() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for paint in [
            CanvasPaint::solid("#b316cd").unwrap(),
            CanvasPaint::Transparent,
        ] {
            for fallback in [false, true] {
                let css = if matches!(paint, CanvasPaint::Transparent) {
                    "transparent"
                } else {
                    "#b316cd"
                };
                let patch = if fallback {
                    ThemeStylePatch::default().with_fill(paint.clone())
                } else {
                    ThemeStylePatch::default().with_stroke(paint.clone())
                };
                let mut rule = ThemeRule::new(ThemeTarget::Edge, patch);
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let theme = block_edge_theme([rule]);
                let rendered = render_block_with_theme_and_engine(
                    "block\n A --> B\n B --> C\n",
                    &theme,
                    MermaidEngine::new(),
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let edges = document
                    .descendants()
                    .filter(|node| node.attribute("data-edge") == Some("true"))
                    .collect::<Vec<_>>();
                assert_eq!(edges.len(), 2);
                for edge in edges {
                    assert_eq!(edge.tag_name().name(), "path");
                    assert!(
                        edge.attribute("style")
                            .unwrap()
                            .contains(&format!("stroke:{css};fill:none;"))
                    );
                    assert!(!edge.attribute("d").unwrap().is_empty());
                }
                for marker in document
                    .descendants()
                    .filter(|node| node.has_tag_name("marker"))
                {
                    assert!(marker.descendants().all(|node| {
                        node.attribute("style")
                            .is_none_or(|style| !style.contains(css))
                    }));
                }
                assert!(
                    !document
                        .descendants()
                        .filter(|node| node.has_tag_name("style"))
                        .any(|node| node.text().unwrap_or("").contains(css))
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), 1);
            }
        }
    }
}

#[test]
fn block_edge_source_config_and_absent_edges_are_not_applicable() {
    let theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
    )]);
    for (source, engine) in [
        (
            "block\n A --> B\n",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"themeVariables":{"lineColor":"#2468ac"}}),
                ),
            ),
        ),
        ("block\n A\n", MermaidEngine::new()),
    ] {
        let rendered = render_block_with_theme_and_engine(source, &theme, engine);
        assert!(!rendered.svg().contains("#b316cd"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
    }
}

#[test]
fn block_edge_ordinal_and_clear_stroke_block_fill_fallback() {
    use merman_render::diagram_theme::Specified;
    for ordinal in [false, true] {
        let mut patch =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap());
        let mut rule = if ordinal {
            ThemeRule::new(ThemeTarget::Edge, patch)
                .with_ordinal(OrdinalSelector::exact(1).unwrap())
        } else {
            patch.stroke.paint = Specified::Clear;
            ThemeRule::new(ThemeTarget::Edge, patch)
        };
        rule = rule.for_family(DiagramFamilyId::BLOCK);
        let theme = block_edge_theme([rule]);
        let rendered = render_block_with_theme_requirement(
            "block\n A --> B\n",
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        assert!(!rendered.svg().contains("stroke:#b316cd"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
    }
}

#[test]
fn block_edge_shadowed_fill_sibling_does_not_hide_winning_stroke() {
    let theme = block_edge_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#abcdef").unwrap())
                .with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
        ),
    ]);
    let rendered =
        render_block_with_theme_and_engine("block\n A --> B\n", &theme, MermaidEngine::new());
    assert!(rendered.svg().contains("stroke:#b316cd;fill:none;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_edge_class_ownership_follows_actual_parent_selector() {
    let theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
    )]);
    for (class, owned) in [("block", true), ("unrelated", false)] {
        let source = format!("block\n A --> B\n classDef {class} stroke:#2468ac\n");
        let rendered = render_block_with_theme_and_engine(&source, &theme, MermaidEngine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let path = document
            .descendants()
            .find(|node| node.attribute("data-edge") == Some("true"))
            .unwrap();
        assert_eq!(
            path.attribute("style")
                .unwrap()
                .contains("stroke:#b316cd;fill:none;"),
            !owned
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(!owned));
        assert_eq!(evidence.not_applicable_count(), usize::from(owned));
    }
}

#[test]
fn block_edge_mixed_rule_only_certifies_consumed_winning_facets() {
    let theme = block_edge_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#b316cd").unwrap())
                .with_stroke_width(4.0)
                .unwrap(),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
        ),
    ]);
    let rendered = render_block_with_theme_requirement(
        "block\n A --> B\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    assert!(rendered.svg().contains("stroke:#2468ac;fill:none;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn block_edge_invisible_self_loop_keeps_rendering_without_applied_paint() {
    for mixed in [false, true] {
        let theme = block_edge_theme([ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
        )]);
        let source = if mixed {
            "block\n A --> A\n A --> B\n"
        } else {
            "block\n A --> A\n"
        };
        let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
                .count(),
            if mixed { 2 } else { 1 }
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(mixed));
        assert_eq!(evidence.not_applicable_count(), usize::from(!mixed));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn block_edge_unsupported_gradient_stroke_blocks_solid_fill_fallback() {
    use merman_render::diagram_theme::{GradientStop, LinearGradient};

    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#b316cd").unwrap())
            .with_stroke(CanvasPaint::LinearGradient(gradient)),
    )]);
    let rendered = render_block_with_theme_requirement(
        "block\n A --> B\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap();
    assert_eq!(edge.attribute("style"), Some("undefined;;;undefined"));
    assert!(!rendered.svg().contains("#b316cd"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn block_edge_invisible_self_loop_preserves_semantic_ordinals() {
    for ordinal in [1, 2] {
        let theme = block_edge_theme([ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(ordinal).unwrap())]);
        let rendered = render_block_with_theme_requirement(
            "block\n A --> A\n A --> B\n",
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let edges = document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect::<Vec<_>>();
        assert_eq!(edges.len(), 2);
        assert!(
            edges
                .iter()
                .all(|edge| { edge.attribute("style") == Some("undefined;;;undefined") })
        );
        assert!(!rendered.svg().contains("#b316cd"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), usize::from(ordinal == 1));
        assert_eq!(evidence.theme_residual_count(), usize::from(ordinal == 2));
    }
}

const BLOCK_CLUSTER_SOURCE: &str =
    "block\n columns 1\n block:group[\"Group\"]\n A[\"Alpha\"]\n end\n B[\"Beta\"]\n";

fn block_cluster_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    block_edge_theme(rules)
}

#[test]
fn block_cluster_label_scalar_reaches_only_the_composite_label_writer() {
    for html in [false, true] {
        let config = serde_json::json!({"htmlLabels":html});
        let baseline = render_block_svg_from_text_with_engine(
            &MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config.clone()),
            ),
            BLOCK_CLUSTER_SOURCE,
        );
        let baseline_document = roxmltree::Document::parse(&baseline).unwrap();
        let baseline_stylesheet = baseline_document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        for variant in [None, Some(ThemeVariant::Default)] {
            for (paint, css) in [
                (CanvasPaint::solid("#b316cd").unwrap(), "#b316cd"),
                (CanvasPaint::Transparent, "transparent"),
            ] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::ClusterLabel,
                    ThemeStylePatch::default().with_fill(paint),
                );
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let theme = block_cluster_theme([rule]);
                let rendered = render_block_with_theme_and_engine(
                    BLOCK_CLUSTER_SOURCE,
                    &theme,
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            config.clone(),
                        ),
                    ),
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let group = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("block-theme-group"))
                    .unwrap();
                let terminal = group
                    .descendants()
                    .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
                    .unwrap();
                assert_eq!(
                    terminal.attribute("style"),
                    Some(format!("{}:{css};", if html { "color" } else { "fill" }).as_str())
                );
                assert_eq!(
                    terminal
                        .descendants()
                        .filter_map(|node| node.text().filter(|_| node.is_text()))
                        .collect::<String>(),
                    "Group"
                );
                let stylesheet = document
                    .descendants()
                    .find(|node| node.has_tag_name("style"))
                    .unwrap()
                    .text()
                    .unwrap();
                assert_eq!(
                    stylesheet.replace("block-theme", "merman"),
                    baseline_stylesheet,
                    "only the existing label terminal receives typed paint"
                );
                let restored = if html {
                    rendered
                        .svg()
                        .replacen(&format!(r#"<p style="color:{css};">"#), "<p>", 1)
                } else {
                    rendered.svg().replacen(&format!("fill:{css};"), "", 1)
                };
                assert_eq!(
                    restored.replace("block-theme", "merman"),
                    baseline,
                    "unpainted output retains the default SVG bytes"
                );
                for id in ["block-theme-A", "block-theme-B"] {
                    let ordinary = document
                        .descendants()
                        .find(|node| node.attribute("id") == Some(id))
                        .unwrap();
                    assert!(
                        !ordinary
                            .descendants()
                            .any(|node| node.attribute("style").unwrap_or("").contains(css))
                    );
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 0);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn block_cluster_label_source_color_follows_the_terminal_and_config_property() {
    let theme = block_cluster_theme([ThemeRule::new(
        ThemeTarget::ClusterLabel,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    )]);
    for html in [false, true] {
        for (suffix, variables, native_applied, html_applied, source_class) in [
            (
                "style group color:#123456\n",
                serde_json::json!({}),
                false,
                true,
                None,
            ),
            (
                "classDef brand color:#123456\nclass group brand\n",
                serde_json::json!({}),
                false,
                true,
                Some("brand"),
            ),
            (
                "classDef nodeLabel color:#123456\n",
                serde_json::json!({}),
                true,
                false,
                Some("nodeLabel"),
            ),
            (
                "",
                serde_json::json!({"nodeTextColor":"#123456"}),
                false,
                false,
                None,
            ),
            (
                "",
                serde_json::json!({"titleColor":"#123456"}),
                true,
                true,
                None,
            ),
        ] {
            let applied = if html { html_applied } else { native_applied };
            let source = format!("{BLOCK_CLUSTER_SOURCE}{suffix}");
            let rendered = render_block_with_theme_and_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"htmlLabels":html,"themeVariables":variables}),
                    ),
                ),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let group = document
                .descendants()
                .find(|node| node.attribute("id") == Some("block-theme-group"))
                .unwrap();
            let terminal = group
                .descendants()
                .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
                .unwrap();
            if applied {
                assert_eq!(
                    terminal.attribute("style"),
                    Some(if html {
                        "color:#b316cd;"
                    } else {
                        "fill:#b316cd;"
                    })
                );
            } else if let Some(class) = source_class {
                assert_block_source_class_reaches_label_terminal(
                    &document, terminal, class, "color", "#123456",
                );
            } else if suffix.starts_with("style ") {
                assert_eq!(terminal.attribute("style"), Some("fill:#123456;"));
            } else {
                // This existing baseline selector directly owns the native text and HTML p.
                assert!(
                    block_stylesheet_text(rendered.svg()).contains(
                        "#block-theme .label text,#block-theme span,#block-theme p{fill:#123456;color:#123456;}"
                    )
                );
                if !html {
                    assert_eq!(
                        terminal.parent_element().unwrap().attribute("class"),
                        Some("label")
                    );
                }
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(applied));
            assert_eq!(evidence.not_applicable_count(), usize::from(!applied));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn block_cluster_label_role_keeps_generic_text_on_other_nodes() {
    for role_last in [false, true] {
        let role = ThemeRule::new(
            ThemeTarget::ClusterLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        );
        let text = ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
        );
        let theme = block_cluster_theme(if role_last {
            [text, role]
        } else {
            [role, text]
        });
        let rendered =
            render_block_with_theme_and_engine(BLOCK_CLUSTER_SOURCE, &theme, MermaidEngine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for (id, color) in [
            ("block-theme-group", "#b316cd"),
            ("block-theme-A", "#2468ac"),
            ("block-theme-B", "#2468ac"),
        ] {
            let node = document
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .unwrap();
            let label = node
                .descendants()
                .find(|node| node.has_tag_name("p"))
                .unwrap();
            assert_eq!(
                label.attribute("style"),
                Some(format!("color:{color};").as_str())
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 2);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn block_cluster_label_reconciles_empty_labels_and_unsupported_selectors() {
    for (source, variant, ordinal, residual) in [
        (
            "block\n block:group[\" \"]\n A[\"Alpha\"]\n end\n",
            None,
            None,
            0,
        ),
        (BLOCK_CLUSTER_SOURCE, Some(ThemeVariant::Active), None, 0),
        (BLOCK_CLUSTER_SOURCE, None, Some(1), 1),
        (BLOCK_CLUSTER_SOURCE, None, Some(2), 0),
    ] {
        let mut rule = ThemeRule::new(
            ThemeTarget::ClusterLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
        );
        if let Some(variant) = variant {
            rule = rule.with_variant(variant);
        }
        if let Some(ordinal) = ordinal {
            rule = rule.with_ordinal(OrdinalSelector::exact(ordinal).unwrap());
        }
        let theme = block_cluster_theme([rule]);
        let rendered = render_block_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let group = document
            .descendants()
            .find(|node| node.attribute("id") == Some("block-theme-group"))
            .unwrap();
        assert!(!group.descendants().any(|node| {
            node.attribute("style")
                .unwrap_or("")
                .contains("color:transparent;")
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), usize::from(residual == 0));
        assert_eq!(evidence.theme_residual_count(), residual);
    }
}

#[test]
fn block_cluster_static_paint_reaches_only_composite_shells() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for transparent in [false, true] {
            let paint = if transparent {
                CanvasPaint::Transparent
            } else {
                CanvasPaint::solid("#b316cd").unwrap()
            };
            let mut rule = ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default()
                    .with_fill(paint.clone())
                    .with_stroke(paint),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = block_cluster_theme([rule]);
            let rendered = render_block_with_theme_and_engine(
                BLOCK_CLUSTER_SOURCE,
                &theme,
                MermaidEngine::new(),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let css = if transparent {
                "transparent"
            } else {
                "#b316cd"
            };
            let cluster = document
                .descendants()
                .find(|node| {
                    node.attribute("class")
                        .unwrap_or("")
                        .split_whitespace()
                        .any(|class| class == "composite")
                })
                .unwrap();
            assert_eq!(cluster.tag_name().name(), "rect");
            assert!(
                cluster
                    .attribute("style")
                    .unwrap()
                    .contains(&format!("fill:{css};stroke:{css};"))
            );
            let ordinary = document.descendants().filter(|node| {
                node.has_tag_name("rect")
                    && node
                        .attribute("class")
                        .unwrap_or("")
                        .contains("label-container")
                    && *node != cluster
            });
            assert!(
                ordinary
                    .into_iter()
                    .all(|node| !node.attribute("style").unwrap_or("").contains(css))
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1);
        }
    }
}

#[test]
fn block_cluster_node_priority_is_resolved_per_paint_property() {
    for split in [false, true] {
        let node_patch = if split {
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap())
        } else {
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#2468ac").unwrap())
                .with_stroke(CanvasPaint::solid("#2468ac").unwrap())
        };
        let theme = block_cluster_theme([
            ThemeRule::new(ThemeTarget::Node, node_patch),
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                    .with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
            ),
        ]);
        let rendered =
            render_block_with_theme_and_engine(BLOCK_CLUSTER_SOURCE, &theme, MermaidEngine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = document
            .descendants()
            .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
            .unwrap();
        let style = cluster.attribute("style").unwrap();
        assert!(style.contains("fill:#2468ac;"));
        assert!(style.contains(if split {
            "stroke:#b316cd;"
        } else {
            "stroke:#2468ac;"
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), if split { 2 } else { 1 });
        assert_eq!(evidence.not_applicable_count(), usize::from(!split));
    }
}

#[test]
fn block_cluster_source_and_explicit_config_own_their_properties() {
    for source_style in [false, true] {
        let theme = block_cluster_theme([ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                .with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
        )]);
        let source = if source_style {
            format!("{BLOCK_CLUSTER_SOURCE}style group fill:#abcdef,stroke:#123456\n")
        } else {
            BLOCK_CLUSTER_SOURCE.to_string()
        };
        let engine = if source_style {
            MermaidEngine::new()
        } else {
            MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({"themeVariables":{"clusterBkg":"#abcdef","clusterBorder":"#123456"}})))
        };
        let rendered = render_block_with_theme_and_engine(&source, &theme, engine);
        assert!(!rendered.svg().contains("fill:#b316cd;"));
        assert!(!rendered.svg().contains("stroke:#2468ac;"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_cluster_empty_domain_is_not_applicable_and_unsupported_winners_stay_residual() {
    for case in ["empty", "mixed", "ordinal"] {
        let mut patch =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap());
        if case == "mixed" {
            patch = patch.with_stroke_width(4.0).unwrap();
        }
        let mut rule = ThemeRule::new(ThemeTarget::Cluster, patch);
        if case == "ordinal" {
            rule = rule.with_ordinal(OrdinalSelector::exact(1).unwrap());
        }
        let theme = block_cluster_theme([rule]);
        let rendered = render_block_with_theme_requirement(
            if case == "empty" {
                "block\n A\n"
            } else {
                BLOCK_CLUSTER_SOURCE
            },
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(case == "empty")
        );
        assert_eq!(
            evidence.theme_residual_count(),
            usize::from(case != "empty")
        );
    }
}

#[test]
fn block_cluster_class_source_and_config_masks_are_property_local() {
    for case in ["class", "cluster-config", "node-config"] {
        let theme = block_cluster_theme([
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
            ),
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                    .with_stroke(CanvasPaint::solid("#123456").unwrap()),
            ),
        ]);
        let source = if case == "class" {
            format!("{BLOCK_CLUSTER_SOURCE}classDef brand fill:#abcdef\nclass group brand\n")
        } else {
            BLOCK_CLUSTER_SOURCE.to_string()
        };
        let engine = if case == "cluster-config" {
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"themeVariables":{"clusterBkg":"#abcdef"}}),
                ),
            )
        } else if case == "node-config" {
            MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"themeVariables":{"mainBkg":"#abcdef", "nodeBorder":"#abcdef"}}),
            ))
        } else {
            MermaidEngine::new()
        };
        let rendered = render_block_with_theme_and_engine(&source, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = document
            .descendants()
            .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
            .unwrap();
        let style = cluster.attribute("style").unwrap();
        assert_eq!(style.contains("fill:#b316cd;"), case == "node-config");
        assert!(style.contains(if case == "node-config" {
            "stroke:#123456;"
        } else {
            "stroke:#2468ac;"
        }));
        let ordinary = document.descendants().filter(|node| {
            node.has_tag_name("rect")
                && node
                    .attribute("class")
                    .unwrap_or("")
                    .contains("label-container")
                && *node != cluster
        });
        assert!(ordinary.into_iter().all(|node| {
            node.attribute("style")
                .unwrap_or("")
                .contains("stroke:#2468ac;")
                == (case != "node-config")
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_cluster_fallback_preserves_unsupported_node_gradient_residual() {
    use merman_render::diagram_theme::{GradientStop, LinearGradient};
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let theme = block_cluster_theme([
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        ),
    ]);
    for source in [
        "block\n block:group[\"Group\"]\n end\n",
        BLOCK_CLUSTER_SOURCE,
    ] {
        let rendered = render_block_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = document
            .descendants()
            .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
            .unwrap();
        assert!(
            cluster
                .attribute("style")
                .unwrap()
                .contains("fill:#b316cd;")
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 1);
    }
}

#[test]
fn block_cluster_structural_node_class_owns_actual_shell_paint() {
    let theme = block_cluster_theme([ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#b316cd").unwrap())
            .with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
    )]);
    let source = format!("{BLOCK_CLUSTER_SOURCE}classDef node fill:#abcdef,stroke:#123456\n");
    let rendered = render_block_with_theme_and_engine(&source, &theme, MermaidEngine::new());
    assert!(
        block_stylesheet_text(rendered.svg())
            .contains(".node>*{fill:#abcdef!important;stroke:#123456!important;}")
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let cluster = document
        .descendants()
        .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
        .unwrap();
    assert!(!cluster.attribute("style").unwrap().contains("#b316cd"));
    assert!(!cluster.attribute("style").unwrap().contains("#2468ac"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_cluster_and_node_ignore_default_class_when_plain_is_assigned() {
    let theme = block_cluster_theme([
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        ),
    ]);
    let source = format!(
        "{BLOCK_CLUSTER_SOURCE}classDef default fill:#abcdef,stroke:#123456\nclassDef plain color:#000000\nclass group,B plain\n"
    );
    let rendered = render_block_with_theme_and_engine(&source, &theme, MermaidEngine::new());
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let cluster = document
        .descendants()
        .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
        .unwrap();
    assert!(
        cluster
            .attribute("style")
            .unwrap()
            .contains("fill:#b316cd;stroke:#2468ac;")
    );
    assert!(terminal_node_has_class(
        rendered.svg(),
        "block-theme-group",
        "plain"
    ));
    assert!(!terminal_node_has_class(
        rendered.svg(),
        "block-theme-group",
        "default"
    ));
    let ordinary = terminal_shell_styles(rendered.svg(), "block-theme-B");
    assert_eq!(terminal_stroke(&ordinary[0].1), Some("#2468ac"));
    let default_owned = terminal_shell_styles(rendered.svg(), "block-theme-A");
    assert!(
        default_owned
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#2468ac"))
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
}

const BLOCK_MARKER_SOURCE: &str = "block-beta\n columns 3\n A[\"Alpha\"] space B[\"Beta\"]\n C[\"Gamma\"] space D[\"Delta\"]\n E[\"Epsilon\"] space F[\"Zeta\"]\n A --> B\n C --o D\n E --x F\n";

fn referenced_block_marker_shapes<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .filter(|node| node.attribute("data-edge") == Some("true"))
        .flat_map(|edge| {
            ["marker-start", "marker-end"]
                .into_iter()
                .filter_map(move |property| {
                    let reference = edge.attribute(property)?;
                    let id = reference.strip_prefix("url(#")?.strip_suffix(')')?;
                    let marker = document
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("marker") && node.attribute("id") == Some(id)
                        })
                        .expect("each emitted marker reference must resolve");
                    Some(
                        marker
                            .children()
                            .find(|node| node.is_element())
                            .expect("marker shape"),
                    )
                })
        })
        .collect()
}

fn marker_style_property(shape: roxmltree::Node<'_, '_>, property: &str) -> Option<String> {
    shape
        .attribute("style")
        .unwrap_or("")
        .split(';')
        .filter_map(|part| part.split_once(':'))
        .filter(|(key, _)| key.trim() == property)
        .map(|(_, value)| value.trim().to_string())
        .next_back()
}

#[test]
fn block_marker_static_paint_uses_independent_real_facets() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for transparent in [false, true] {
            for split in [false, true] {
                let fill = if transparent {
                    CanvasPaint::Transparent
                } else {
                    CanvasPaint::solid("#b316cd").unwrap()
                };
                let stroke = if transparent {
                    CanvasPaint::Transparent
                } else {
                    CanvasPaint::solid("#167bc1").unwrap()
                };
                let patches = if split {
                    vec![
                        ThemeStylePatch::default().with_fill(fill),
                        ThemeStylePatch::default().with_stroke(stroke),
                    ]
                } else {
                    vec![
                        ThemeStylePatch::default()
                            .with_fill(fill)
                            .with_stroke(stroke),
                    ]
                };
                let theme = block_edge_theme(patches.into_iter().map(|patch| {
                    let rule = ThemeRule::new(ThemeTarget::Marker, patch);
                    if let Some(variant) = variant {
                        rule.with_variant(variant)
                    } else {
                        rule
                    }
                }));
                let rendered = render_block_with_theme_and_engine(
                    BLOCK_MARKER_SOURCE,
                    &theme,
                    MermaidEngine::new(),
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let shapes = referenced_block_marker_shapes(&document);
                assert_eq!(shapes.len(), 3);
                for shape in shapes {
                    let cross = shape
                        .parent()
                        .unwrap()
                        .attribute("class")
                        .unwrap_or("")
                        .split_whitespace()
                        .any(|name| name == "cross");
                    if !cross {
                        assert_eq!(
                            marker_style_property(shape, "fill").as_deref(),
                            Some(if transparent {
                                "transparent"
                            } else {
                                "#b316cd"
                            })
                        );
                    }
                    assert_eq!(
                        marker_style_property(shape, "stroke").as_deref(),
                        Some(if transparent {
                            "transparent"
                        } else {
                            "#167bc1"
                        })
                    );
                }
                for edge in document
                    .descendants()
                    .filter(|node| node.attribute("data-edge") == Some("true"))
                {
                    assert!(!edge.attribute("style").unwrap_or("").contains("#167bc1"));
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), if split { 2 } else { 1 });
            }
        }
    }
}

#[test]
fn block_marker_fill_has_no_domain_for_unused_definitions_or_open_crosses() {
    let theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Marker,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    )]);
    for source in [
        "block-beta\n A\n",
        "block-beta\n A --- B\n",
        "block-beta\n A x--x B\n",
    ] {
        let rendered = render_block_with_theme_and_engine(source, &theme, MermaidEngine::new());
        assert!(!rendered.svg().contains("#b316cd"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_marker_stroke_does_not_reuse_the_legacy_fill_projection() {
    let theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Marker,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#167bc1").unwrap()),
    )]);
    let rendered =
        render_block_with_theme_and_engine(BLOCK_MARKER_SOURCE, &theme, MermaidEngine::new());
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    for shape in referenced_block_marker_shapes(&document) {
        assert_eq!(
            marker_style_property(shape, "stroke").as_deref(),
            Some("#167bc1")
        );
        assert_ne!(
            marker_style_property(shape, "fill").as_deref(),
            Some("#167bc1")
        );
    }
    assert!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("style"))
            .all(|node| !node.text().unwrap_or("").contains("#167bc1"))
    );
}

#[test]
fn block_marker_explicit_fill_config_leaves_typed_stroke_available() {
    let theme = block_edge_theme([
        ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#167bc1").unwrap()),
        ),
    ]);
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(
            serde_json::json!({"themeVariables":{"arrowheadColor":"#315724"}}),
        ),
    );
    let rendered = render_block_with_theme_and_engine(BLOCK_MARKER_SOURCE, &theme, engine);
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    for shape in referenced_block_marker_shapes(&document) {
        assert_ne!(
            marker_style_property(shape, "fill").as_deref(),
            Some("#b316cd")
        );
        assert_eq!(
            marker_style_property(shape, "stroke").as_deref(),
            Some("#167bc1")
        );
    }
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_marker_unsupported_ordinal_does_not_repaint_other_references() {
    let theme = block_edge_theme([
        ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#167bc1").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(2).unwrap()),
    ]);
    let rendered = render_block_with_theme_requirement(
        "block-beta\n columns 3\n A space B\n C space D\n A --> B\n C --> D\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let shapes = referenced_block_marker_shapes(&document);
    assert_eq!(shapes.len(), 2);
    assert_eq!(
        marker_style_property(shapes[0], "fill").as_deref(),
        Some("#b316cd")
    );
    assert_ne!(
        marker_style_property(shapes[1], "fill").as_deref(),
        Some("#b316cd")
    );
    assert_ne!(
        marker_style_property(shapes[1], "fill").as_deref(),
        Some("#167bc1")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert!(evidence.theme_residual_count() > 0);
}

#[test]
fn block_look_resources_precede_markers_and_follow_theme_configuration() {
    for (theme, gradient, flood) in [("redux", false, "#000000"), ("redux-dark", true, "#FFFFFF")] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "theme": theme,
                "look": "neo",
                "themeVariables": {
                    "useGradient": gradient,
                    "gradientStart": "#123456",
                    "gradientStop": "#abcdef"
                }
            })),
        );
        let svg = render_block_svg_from_text_with_engine(&engine, "block-beta\n  A[\"Alpha\"]\n");
        let document = roxmltree::Document::parse(&svg).expect("Block SVG");
        let root = document.root_element();
        let diagram_id = root.attribute("id").expect("diagram id");
        let children = root
            .children()
            .filter(|node| node.is_element())
            .collect::<Vec<_>>();
        let marker_index = children
            .iter()
            .position(|node| node.has_tag_name("marker"))
            .expect("marker");
        for (suffix, size, offset) in [
            ("drop-shadow", "130%", "4"),
            ("drop-shadow-small", "150%", "2"),
        ] {
            let id = format!("{diagram_id}-{suffix}");
            let matches = root
                .descendants()
                .filter(|node| node.attribute("id") == Some(id.as_str()))
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "look resources must have unique ids");
            let filter = matches[0];
            assert!(filter.has_tag_name("filter"));
            assert_eq!(filter.attribute("height"), Some(size));
            assert_eq!(filter.attribute("width"), Some(size));
            let defs = filter.parent().unwrap();
            assert!(defs.has_tag_name("defs"));
            assert!(children.iter().position(|node| *node == defs).unwrap() < marker_index);
            let shadow = filter
                .children()
                .find(|node| node.has_tag_name("feDropShadow"))
                .unwrap();
            assert_eq!(shadow.attribute("dx"), Some(offset));
            assert_eq!(shadow.attribute("dy"), Some(offset));
            assert_eq!(shadow.attribute("flood-color"), Some(flood));
        }
        let gradient_node = children
            .iter()
            .find(|node| node.has_tag_name("linearGradient"));
        assert_eq!(gradient_node.is_some(), gradient);
        if let Some(node) = gradient_node {
            assert_eq!(
                node.attribute("id"),
                Some(format!("{diagram_id}-gradient").as_str())
            );
            assert!(children.iter().position(|child| child == node).unwrap() < marker_index);
            let stops = node
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .collect::<Vec<_>>();
            assert_eq!(stops.len(), 2);
            assert_eq!(stops[0].attribute("stop-color"), Some("#123456"));
            assert_eq!(stops[1].attribute("stop-color"), Some("#abcdef"));
        }
    }
}

#[test]
fn block_composite_palette_arrays_own_the_actual_shell_paint() {
    const SOURCE: &str = "block-beta\nblock:g\nA\nend\n";
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Cluster,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#123456").unwrap())
                        .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                )),
            ),
        )
        .unwrap();
    for source_owned in [true, false] {
        let source = if source_owned {
            format!("{SOURCE}style g fill:#112233,stroke:#334455\n")
        } else {
            SOURCE.to_owned()
        };
        let rendered = render_block_with_theme_requirement(
            &source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "theme": "redux-color", "look": "classic",
                        "themeVariables": {
                            "bkgColorArray": ["#aabbcc"], "borderColorArray": ["#445566"]
                        }
                    }),
                ),
            ),
            ThemePortabilityRequirement::BestEffort,
        );
        assert_eq!(
            rendered.metadata().effective_config.get_str("theme"),
            Some("redux-color")
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = document
            .descendants()
            .find(|node| node.attribute("id") == Some("block-theme-g"))
            .expect("visible Block composite");
        assert_eq!(cluster.attribute("data-look"), Some("classic"));
        let shell = cluster
            .children()
            .find(|node| node.has_tag_name("rect"))
            .expect("visible Block composite shell");
        let style = shell.attribute("style").unwrap_or_default();
        if source_owned {
            assert_eq!(terminal_fill(style), Some("#112233"));
            assert_eq!(terminal_stroke(style), Some("#334455"));
        } else {
            let slot = cluster
                .attribute("data-color-id")
                .expect("Block palette slot");
            let selector = format!(
                "#block-theme [data-look=\"classic\"][data-color-id=\"{slot}\"].node rect.composite{{"
            );
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .expect("Block stylesheet");
            let palette_rule = css
                .split_once(&selector)
                .expect("materialized composite palette selector")
                .1
                .split_once('}')
                .unwrap()
                .0;
            assert!(palette_rule.contains("fill:#aabbcc;"), "{palette_rule}");
            assert!(palette_rule.contains("stroke:#445566;"), "{palette_rule}");
            assert_eq!(
                terminal_fill(style),
                None,
                "typed fill must not override the explicit composite palette"
            );
            assert_eq!(
                terminal_stroke(style),
                None,
                "typed stroke must not override the explicit composite palette"
            );
        }
        drop(document);
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
fn block_composite_palette_wraps_slots_and_preserves_author_styles() {
    let source = "block-beta\nblock:outer\nblock:inner\nA\nend\nend\nblock:sibling\nB\nend\nstyle inner fill:#112233,stroke:#445566\n";
    for (theme, tinted) in [
        ("redux-color", true),
        ("redux-dark-color", false),
        ("redux", false),
    ] {
        let engine = MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "theme": theme, "look": "classic",
            "themeVariables": {"borderColorArray": ["#aa0000", "#00aa00"],
                "bkgColorArray": if tinted { vec!["#eeeeee"] } else { vec![] }, "THEME_COLOR_LIMIT": 1}
        })));
        let svg = render_block_svg_from_text_with_engine(&engine, source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let id = document.root_element().attribute("id").unwrap();
        for (name, slot) in [
            ("outer", "color-0"),
            ("inner", "color-1"),
            ("sibling", "color-0"),
            ("A", ""),
            ("B", ""),
        ] {
            let node_id = format!("{id}-{name}");
            let node = document
                .descendants()
                .find(|node| node.attribute("id") == Some(node_id.as_str()))
                .unwrap();
            assert_eq!(node.attribute("data-look"), Some("classic"));
            assert_eq!(
                node.attribute("data-color-id"),
                (theme != "redux" && !slot.is_empty()).then_some(slot)
            );
            if name == "inner" {
                let rect = node
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                let style = rect.attribute("style").unwrap();
                assert!(style.contains("fill:#112233") && style.contains("stroke:#445566"));
            }
        }
        let rule = format!(
            r#"#{id} [data-look="classic"][data-color-id="color-1"].node rect.composite{{stroke:#00aa00;{}}}"#,
            if tinted { "fill:#eeeeee;" } else { "" }
        );
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert_eq!(css.contains(&rule), theme != "redux");
    }
}

#[test]
fn block_flat_records_preserve_nested_dom_order_and_directives() {
    let svg = render_block_svg_from_text(
        "block\ncolumns 2\nblock:outer:2\ncolumns 2\nA[\"Alpha\"]\nblock:inner\nB<[\"Route\"]>(left,down)\nend\nend\nC\nA --> C\nclassDef branded fill:#696,stroke:#333\nclass A branded\nstyle inner fill:#112233,stroke:#445566\n",
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let nodes = document
        .descendants()
        .filter(|node| {
            node.attribute("class").is_some_and(|classes| {
                classes
                    .split_ascii_whitespace()
                    .any(|class| class == "node")
            })
        })
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        nodes,
        [
            "merman-outer",
            "merman-A",
            "merman-inner",
            "merman-B",
            "merman-C"
        ]
    );
    assert!(svg.contains("node branded flowchart-label"));
    assert!(svg.contains("fill:#112233;stroke:#445566"));
    assert!(svg.contains("merman-1-A-C"));
}

#[test]
fn block_uppercase_source_color_reaches_svg_text_fill() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
    );
    let rendered = render_block_with_theme_and_engine(
        "block\n A[\"Alpha\"]\n style A COLOR:#123456\n",
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"htmlLabels":false}),
            ),
        ),
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let text = document
        .descendants()
        .find(|n| n.has_tag_name("text") && n.descendants().any(|n| n.text() == Some("Alpha")))
        .expect("visible SVG node text");
    assert!(
        text.attribute("style")
            .unwrap_or("")
            .contains("fill:#123456;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_source_all_reset_owns_node_and_edge_paint() {
    let node_theme = block_node_fill_and_stroke_theme(
        CanvasPaint::solid("#aabbcc").unwrap(),
        CanvasPaint::solid("#ddeeff").unwrap(),
    );
    for class in ["default", "node"] {
        let rendered = render_block_with_theme_and_engine(
            &format!("block\n A[\"Alpha\"]\n classDef {class} all:unset\n"),
            &node_theme,
            MermaidEngine::new(),
        );
        assert!(
            rendered.svg().contains("all:unset !important")
                || rendered.svg().contains("all:unset!important")
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = document
            .descendants()
            .find(|n| n.attribute("id") == Some("block-theme-A"))
            .unwrap();
        assert!(
            !node
                .descendants()
                .any(|n| n.attribute("style").unwrap_or("").contains("#aabbcc"))
        );
        assert!(
            !node
                .descendants()
                .any(|n| n.attribute("style").unwrap_or("").contains("#ddeeff"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0, "class={class}");
        assert_eq!(evidence.not_applicable_count(), 1, "class={class}");
        assert_eq!(evidence.theme_residual_count(), 0, "class={class}");
    }
    let edge_theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#aabbcc").unwrap()),
    )]);
    let rendered = render_block_with_theme_and_engine(
        "block\n A --> B\n classDef block all:unset\n",
        &edge_theme,
        MermaidEngine::new(),
    );
    assert!(
        rendered.svg().contains("all:unset !important")
            || rendered.svg().contains("all:unset!important")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_html_source_font_longhands_measure_the_emitted_terminal() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::TextMetrics;
    use std::sync::Arc;

    struct FontHost;
    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::Wrapped || request.text != "Alpha" {
                return Ok(None);
            }
            let family_width = if request.style.font_family.as_deref() == Some("monospace") {
                7.0
            } else {
                0.0
            };
            let italic_width = if request.style.font_style.as_deref() == Some("italic") {
                5.0
            } else {
                0.0
            };
            Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                width: request.style.font_size * 2.0 + family_width + italic_width,
                height: request.style.font_size * 1.5,
                line_count: 1,
            })))
        }
    }

    for (size, expected_width, expected_height) in [
        ("48px", 108.0, 72.0),
        ("36pt", 108.0, 72.0),
        // The same declaration reaches g.label, the XHTML div, and span.nodeLabel.
        ("150%", 120.0, 81.0),
        ("1.5em", 120.0, 81.0),
    ] {
        for assigned_class in [false, true] {
            let declarations = format!("font-size:{size},font-family:monospace,font-style:italic");
            let source = if assigned_class {
                format!("block\n A[\"Alpha\"]\n classDef face {declarations}\n class A face\n")
            } else {
                format!("block\n A[\"Alpha\"]\n style A {declarations}\n")
            };
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": true, "themeVariables": {"fontSize": "16px"}}),
                ),
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.block-source-terminal-font").unwrap(),
                "1",
            )
            .unwrap();
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                    identity,
                    Arc::new(FontHost),
                    TextMeasurementPhase::ALL,
                ))
                .begin_session()
                .unwrap();
            let artifact =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|node| node.attribute("id") == Some("merman-A"))
                .unwrap();
            let terminal = node
                .descendants()
                .find(|node| node.has_tag_name("p"))
                .unwrap();
            assert_eq!(terminal.text(), Some("Alpha"));
            let styled_ancestors: Vec<_> = terminal
                .ancestors()
                .filter(|node| {
                    node.attribute("style")
                        .is_some_and(|style| style.contains("font-size:"))
                })
                .collect();
            assert_eq!(styled_ancestors.len(), 3, "preserve the pinned style sinks");
            for ancestor in styled_ancestors {
                let style = ancestor.attribute("style").unwrap();
                let priority = " !important";
                assert!(
                    style.contains(&format!("font-size:{size}{priority};")),
                    "{style}"
                );
                assert!(
                    style.contains(&format!("font-family:monospace{priority};")),
                    "{style}"
                );
                assert!(
                    style.contains(&format!("font-style:italic{priority};")),
                    "{style}"
                );
            }
            let foreign_object = terminal
                .ancestors()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            assert_eq!(
                numeric_attribute(foreign_object, "width"),
                expected_width,
                "{size}"
            );
            assert_eq!(
                numeric_attribute(foreign_object, "height"),
                expected_height,
                "{size}"
            );
        }
    }
}

#[test]
fn block_source_font_weight_priority_reaches_labels_and_measurement() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::TextMetrics;
    use std::sync::{Arc, Mutex};
    struct WeightHost(Arc<Mutex<Vec<Option<String>>>>);
    impl HostTextMeasurer for WeightHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation == TextMeasurementOperation::Wrapped && request.text == "Alpha" {
                self.0
                    .lock()
                    .unwrap()
                    .push(request.style.font_weight.clone());
                return Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                    width: if request.style.font_weight.as_deref() == Some("700") {
                        70.0
                    } else {
                        40.0
                    },
                    height: 24.0,
                    line_count: 1,
                })));
            }
            Ok(None)
        }
    }
    for html in [false, true] {
        for (later, expected_css, expected_weight) in [
            ("FONT-WEIGHT:normal", "font-weight:bold !important;", "700"),
            (
                "FONT-WEIGHT:normal !important",
                "FONT-WEIGHT:normal !important;",
                "400",
            ),
        ] {
            let source = format!(
                "%%{{init: {{\"htmlLabels\": {html}}}}}%%\nblock\n A[\"Alpha\"]\n style A font-weight:bold !important,{later}\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let weights = Arc::new(Mutex::new(Vec::new()));
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.block-source-weight-priority").unwrap(),
                "1",
            )
            .unwrap();
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                    identity,
                    Arc::new(WeightHost(weights.clone())),
                    TextMeasurementPhase::ALL,
                ))
                .begin_session()
                .unwrap();
            let artifact =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|n| n.attribute("id") == Some("merman-A"))
                .unwrap();
            let label = node
                .descendants()
                .find(|n| n.has_tag_name("g") && n.attribute("class") == Some("label"))
                .unwrap();
            assert_eq!(label.attribute("style"), Some(expected_css));
            let terminal = label
                .descendants()
                .find(|n| {
                    if html {
                        n.has_tag_name("span")
                    } else {
                        n.has_tag_name("text")
                    }
                })
                .unwrap();
            assert_eq!(terminal.attribute("style"), Some(expected_css));
            if !html {
                let word = terminal
                    .descendants()
                    .find(|node| node.attribute("class") == Some("text-inner-tspan"))
                    .unwrap();
                assert_eq!(word.attribute("font-weight"), Some("normal"));
                assert_eq!(word.attribute("font-style"), Some("normal"));
            }
            let measured = weights.lock().unwrap();
            assert!(!measured.is_empty());
            assert!(
                measured.iter().all(|weight| {
                    if html {
                        weight.as_deref() == Some(expected_weight)
                    } else {
                        matches!(weight.as_deref(), None | Some("normal" | "400"))
                    }
                }),
                "html={html}, {later}: {measured:?}"
            );
        }
    }
}

#[test]
fn block_class_fonts_follow_assignment_order_and_terminal_inline_priority() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMetrics, TextStyle};
    use std::sync::{Arc, Mutex};

    struct FontHost(Arc<Mutex<Vec<TextStyle>>>);
    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::Wrapped || request.text != "Alpha" {
                return Ok(None);
            }
            self.0.lock().unwrap().push(request.style.clone());
            Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                width: request.style.font_size * 2.0,
                height: request.style.font_size * 1.5,
                line_count: 1,
            })))
        }
    }

    for (declarations, expected_size, expected_family) in [
        (
            "classDef default font-size:36pt,font-family:monospace\n",
            48.0,
            "monospace",
        ),
        (
            "classDef default font-size:150%,font-family:monospace\n",
            36.0,
            "monospace",
        ),
        // Class assignment order wins the compiled inline label, independent of definition order.
        (
            "classDef first font-size:24px,font-family:serif\nclassDef second font-size:48px,font-family:monospace\nclass A second\nclass A first\n",
            24.0,
            "serif",
        ),
        (
            "classDef first font-size:24px,font-family:serif\nclassDef second font-size:48px,font-family:monospace\nclass A first\nclass A second\n",
            48.0,
            "monospace",
        ),
        (
            "classDef face font-size:48px,font-family:monospace\nclass A face\nstyle A font-size:30px,font-family:serif\n",
            30.0,
            "serif",
        ),
        (
            "classDef face font-size:48px,font-family:monospace\nclass A face\nstyle A font-size:30px !important,font-family:serif !important\n",
            30.0,
            "serif",
        ),
        // Direct styles are compiled to inline important fonts even without an assigned class.
        (
            "classDef default font-size:48px,font-family:monospace\nstyle A font-size:30px,font-family:serif\n",
            30.0,
            "serif",
        ),
        (
            "classDef default font-size:48px,font-family:monospace\nstyle A font-size:30px !important,font-family:serif !important\n",
            30.0,
            "serif",
        ),
        (
            "classDef default font-size:48px\nclassDef plain stroke:red\nclass A plain\nstyle A font-size:30px,font-family:serif\n",
            30.0,
            "serif",
        ),
    ] {
        let source = format!("block\nA[\"Alpha\"]\n{declarations}");
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"htmlLabels": true, "themeVariables": {"fontSize": "16px"}}),
            ),
        );
        let parsed = engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.block-class-font-order").unwrap(),
            "1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                Arc::new(FontHost(calls.clone())),
                TextMeasurementPhase::ALL,
            ))
            .begin_session()
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-A"))
            .unwrap();
        let viewport = node
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            numeric_attribute(viewport, "width"),
            expected_size * 2.0,
            "{declarations}"
        );
        assert_eq!(
            numeric_attribute(viewport, "height"),
            expected_size * 1.5,
            "{declarations}"
        );
        let calls = calls.lock().unwrap();
        assert!(!calls.is_empty(), "{declarations}");
        assert!(
            calls.iter().all(|style| style.font_size == expected_size
                && style.font_family.as_deref() == Some(expected_family)),
            "{declarations}: {calls:?}"
        );
        if declarations.contains("class A face") {
            let span = node
                .descendants()
                .find(|node| node.attribute("class") == Some("nodeLabel"))
                .unwrap();
            assert!(
                span.attribute("style")
                    .unwrap()
                    .contains("font-size:30px !important;")
            );
        }
    }
}

#[test]
fn block_html_wrapper_class_fonts_reach_multiline_and_descendant_measurement() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use std::sync::{Arc, Mutex};

    struct FontHost(Arc<Mutex<Vec<(String, f64)>>>);
    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if !matches!(request.text, "Alpha" | "Beta") {
                return Ok(None);
            }
            let length = match request.operation {
                TextMeasurementOperation::CanvasMeasureTextWidth => request.style.font_size * 2.0,
                TextMeasurementOperation::HtmlLineHeight => request.style.font_size * 1.5,
                _ => return Ok(None),
            };
            self.0
                .lock()
                .unwrap()
                .push((request.text.to_owned(), request.style.font_size));
            Ok(Some(HostTextMeasurement::Length(length)))
        }
    }

    for (label, expected_size, expected_height) in [
        ("Alpha<br/>Beta", 48.0, 144.0),
        ("<span style='font-size:24px'>Alpha</span>", 48.0, 72.0),
        (
            "<span style='font-size:24px !important'>Alpha</span>",
            24.0,
            72.0,
        ),
    ] {
        let source = format!("block\nA[\"{label}\"]\nclassDef nodeLabel font-size:48px\n");
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels":true,"themeVariables":{"fontSize":"16px"}}),
                ),
            )
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.block-wrapper-class-terminal").unwrap(),
            "1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                Arc::new(FontHost(calls.clone())),
                TextMeasurementPhase::ALL,
            ))
            .begin_session()
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-A"))
            .unwrap();
        let viewport = node
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            numeric_attribute(viewport, "width"),
            expected_size * 2.0,
            "{label}"
        );
        assert_eq!(
            numeric_attribute(viewport, "height"),
            expected_height,
            "{label}"
        );
        let paragraph = node
            .descendants()
            .find(|node| node.has_tag_name("p"))
            .unwrap();
        assert!(
            paragraph.attribute("style").is_none(),
            "measurement projection must not rewrite the emitted DOM"
        );
        assert!(
            rendered
                .svg()
                .contains(".nodeLabel>*{font-size:48px!important;}")
        );
        assert!(
            rendered
                .svg()
                .contains(".nodeLabel span{font-size:48px!important;}")
        );
        let calls = calls.lock().unwrap();
        assert!(
            calls.iter().any(|(text, _)| text == "Alpha"),
            "{label}: {calls:?}"
        );
        if label.contains("Beta") {
            assert!(calls.iter().any(|(text, _)| text == "Beta"), "{calls:?}");
        }
        assert!(
            calls.iter().all(|(_, size)| *size == expected_size),
            "{label}: {calls:?}"
        );
    }
}

#[test]
fn block_unknown_wrapper_descendant_font_remains_unverified_over_inline_absolute_size() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::BLOCK,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Block Face").unwrap()),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    let source = "block\nA[\"<span style='font-size:24px'>Visible</span>\"]\nclassDef nodeLabel font-size:var(--host-size)\n";
    let parsed = common::install_parse_compatibility(
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"htmlLabels":true,"themeVariables":{"fontSize":"0px"}}),
            ),
        ),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .unwrap()
    .unwrap()
    .into_presentation();
    let rendered = render_block_presentation_with_theme(
        &parsed,
        &theme,
        ThemePortabilityRequirement::BestEffort,
    );
    assert!(
        rendered
            .svg()
            .contains(".nodeLabel span{font-size:var(--host-size)!important;}")
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(document.descendants().any(|node| {
        node.has_tag_name("span")
            && node
                .attribute("style")
                .is_some_and(|style| style.contains("font-size:24px"))
    }));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(
        evidence.not_applicable_count(),
        0,
        "a zero measurement fallback cannot prove hidden text"
    );
    assert_eq!(evidence.theme_residual_count(), 2);
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .unwrap();
    let result = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
    assert!(
        matches!(
            result,
            Err(merman_render::Error::UnverifiedFamilyTheme {
                family_id: DiagramFamilyId::BLOCK,
                residual_count: 2,
            })
        ),
        "unknown important descendant CSS must retain the Block portability residual"
    );
}

#[test]
fn block_zero_fonts_retire_text_evidence_without_removing_shells_or_backgrounds() {
    let typography =
        ThemeTextStyle::default().with_font_stack(FontStack::single("Block Face").unwrap());
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(DiagramFamilyId::BLOCK, typography),
                )
                .with_styles(
                    [
                        ThemeTarget::Node,
                        ThemeTarget::NodeLabel,
                        ThemeTarget::ClusterLabel,
                        ThemeTarget::EdgeLabelBackground,
                    ]
                    .into_iter()
                    .fold(ThemeRuleSet::default(), |rules, target| {
                        rules.with_rule(ThemeRule::new(
                            target,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#b316cd").unwrap()),
                        ))
                    }),
                ),
        )
        .unwrap();
    for html in [false, true] {
        for from_source in [false, true] {
            let config =
                serde_json::json!({"htmlLabels": html, "themeVariables": {"fontSize": "0px"}});
            let body = "block\ncolumns 2\nblock:group:2\nA[\"Alpha\"] B[\"Beta\"]\nend\nA -- \"relates\" --> B\n";
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config)
            };
            let rendered = render_block_with_theme_and_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            assert!(css.contains("font-size:0px;"), "{css}");
            assert!(
                css.contains(".edgeLabel{background-color:#b316cd;"),
                "{css}"
            );
            for id in ["block-theme-group", "block-theme-A", "block-theme-B"] {
                let node = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .unwrap();
                let shell = node
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .unwrap();
                assert!(numeric_attribute(shell, "width") > 0.0);
                assert!(numeric_attribute(shell, "height") > 0.0);
                assert!(shell.attribute("style").unwrap().contains("fill:#b316cd;"));
            }
            if !html {
                assert!(document.descendants().any(|node| node.has_tag_name("rect")
                    && node.attribute("class") == Some("background")
                    && numeric_attribute(node, "width") > 0.0));
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(
                evidence.required_count(),
                5,
                "html={html}, source={from_source}"
            );
            assert_eq!(
                evidence.applied_count(),
                2,
                "shell and background still consume paint"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                3,
                "zero text cannot consume fonts or fill"
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn block_subpixel_config_and_typed_sizes_reach_css_and_layout() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::TextMetrics;
    use std::sync::{Arc, Mutex};

    struct FontHost(Arc<Mutex<Vec<(String, f64)>>>);
    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::Wrapped
                || !matches!(request.text, "Alpha" | "Beta" | "relates")
            {
                return Ok(None);
            }
            self.0
                .lock()
                .unwrap()
                .push((request.text.to_owned(), request.style.font_size));
            Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                width: request.style.font_size * 2.0,
                height: request.style.font_size * 1.5,
                line_count: 1,
            })))
        }
    }

    for html in [false, true] {
        for (config_size, typed_size, expected) in
            [(None, 0.5, 0.5), (Some("0.5pt"), 22.0, 2.0 / 3.0)]
        {
            let theme = block_typography_theme("Block Face", typed_size);
            let mut config = serde_json::json!({"htmlLabels":html});
            if let Some(size) = config_size {
                config["themeVariables"] = serde_json::json!({"fontSize":size});
            }
            let parsed = common::install_parse_compatibility(
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                ),
            )
            .parse_diagram_for_render_model_sync(
                "block\nA[\"Alpha\"] B[\"Beta\"]\nA -- \"relates\" --> B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.block-subpixel-terminal").unwrap(),
                "1",
            )
            .unwrap();
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                    identity,
                    Arc::new(FontHost(calls.clone())),
                    TextMeasurementPhase::ALL,
                ))
                .begin_session_with_theme(&theme)
                .unwrap();
            let artifact =
                family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let alpha = document
                .descendants()
                .find(|node| node.attribute("id") == Some("merman-A"))
                .expect("Alpha is emitted from the prepared layout");
            let label = alpha
                .children()
                .find(|node| node.attribute("class") == Some("label"))
                .expect("Alpha label wrapper");
            // Public layout JSON deliberately skips the internal label dimensions.
            // The emitted translation consumes that same measured height in both modes.
            let (x, y) = translated_center(label);
            assert!(
                (y + expected * 1.5 / 2.0).abs() < 0.000001,
                "html={html}: {y}"
            );
            assert!((x - if html { -expected } else { 0.0 }).abs() < 0.000001);
            if html {
                let viewport = label
                    .descendants()
                    .find(|node| node.has_tag_name("foreignObject"))
                    .expect("HTML label viewport");
                assert!((numeric_attribute(viewport, "width") - expected * 2.0).abs() < 0.000001);
                assert!((numeric_attribute(viewport, "height") - expected * 1.5).abs() < 0.000001);
            }
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            let size = css
                .split("font-size:")
                .nth(1)
                .unwrap()
                .split("px")
                .next()
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert!((size - expected).abs() < 0.000001, "{css}");
            let calls = calls.lock().unwrap();
            for label in ["Alpha", "Beta", "relates"] {
                assert!(
                    calls.iter().any(|(text, _)| text == label),
                    "{label}: {calls:?}"
                );
            }
            assert!(
                calls
                    .iter()
                    .all(|(_, size)| (size - expected).abs() < 0.000001),
                "{calls:?}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(
                evidence.applied_count(),
                2 - usize::from(config_size.is_some())
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(config_size.is_some())
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn block_local_zero_fonts_and_unknown_host_sizes_have_distinct_evidence() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::BLOCK,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Block Face").unwrap()),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    for html in [false, true] {
        for (style, unknown) in [
            ("0px", false),
            ("0pt", false),
            ("var(--host-size)", true),
            ("1rem", true),
        ] {
            for owner in ["direct", "default", "assigned"] {
                let declarations = match owner {
                    "direct" => format!("style A font-size:{style}\n"),
                    "default" => format!("classDef default font-size:{style}\n"),
                    _ => format!("classDef face font-size:{style}\nclass A face\n"),
                };
                let source = format!("block\nA[\"Alpha\"]\n{declarations}");
                let rendered = render_block_with_theme_requirement(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"htmlLabels":html, "themeVariables":{"fontSize":"0px"}}),
                    ),
                ),
                ThemePortabilityRequirement::BestEffort,
            );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), 0, "{html}/{style}");
                assert_eq!(
                    evidence.not_applicable_count(),
                    if unknown { 0 } else { 2 },
                    "{html}/{style}"
                );
                assert_eq!(
                    evidence.theme_residual_count(),
                    if unknown { 2 } else { 0 },
                    "{html}/{style}"
                );
            }
        }
    }
}

#[test]
fn block_inline_absolute_text_can_escape_a_zero_parent_without_reviving_hidden_siblings() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::BLOCK,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Block Face").unwrap()),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    for (label, visible) in [
        ("<span style='font-size:0px'>Hidden</span>", false),
        ("<span style='font-size:2em'>Hidden</span>", false),
        (
            "<span style='font-size:24px'>Visible</span><span style='font-size:0px'>Hidden</span>",
            true,
        ),
    ] {
        let source = format!("block\nA[\"{label}\"]\n");
        let rendered = render_block_with_theme_and_engine(
            &source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels":true,"themeVariables":{"fontSize":"0px"}}),
                ),
            ),
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(
            evidence.applied_count(),
            if visible { 2 } else { 0 },
            "{label}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            if visible { 0 } else { 2 },
            "{label}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{label}");
    }
}

#[test]
fn block_zero_edge_label_class_keeps_its_background_without_font_evidence() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::BLOCK,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Block Face").unwrap()),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::EdgeLabelBackground,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))),
        )
        .unwrap();
    for html in [false, true] {
        let rendered = render_block_with_theme_and_engine(
            "block\nA[\"\"] -- \"relates\" --> B[\"\"]\nclassDef edgeLabel font-size:0px\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels":html}),
                ),
            ),
        );
        assert!(
            rendered
                .svg()
                .contains(".edgeLabel{background-color:#123456;")
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(
            evidence.applied_count(),
            1,
            "background consumes paint: html={html}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            1,
            "zero edge label cannot prove the font: html={html}"
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}
