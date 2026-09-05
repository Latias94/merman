use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, GradientStop,
    LinearGradient, OrdinalPalette, OrdinalSelector, PatternKind, PatternSpec, RadialGradient,
    Specified, ThemeColorValue, ThemeGeometryPatch, ThemeLength, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant,
    TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::C4DiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};

fn render_c4_svg_with_environment(source: &str, environment: &RenderEnvironment) -> String {
    try_render_c4_svg_with_environment(source, environment).expect("render C4 SVG")
}

fn try_render_c4_svg_with_environment(
    source: &str,
    environment: &RenderEnvironment,
) -> merman_render::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let session = environment.begin_session().expect("begin render session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;

    Ok(artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?
        .svg()
        .to_owned())
}

fn c4_cluster_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile C4 cluster theme")
}

fn c4_cluster_radius_theme(radius: Specified<f32>) -> DiagramTheme {
    let style = ThemeStylePatch {
        geometry: ThemeGeometryPatch { radius },
        ..ThemeStylePatch::default()
    };
    c4_cluster_rules_theme([
        ThemeRule::new(ThemeTarget::Cluster, style).for_family(merman_render::DiagramFamilyId::C4)
    ])
}

fn c4_cluster_paint_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    c4_cluster_rules_theme([ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default()
            .with_fill(fill)
            .with_stroke(stroke),
    )
    .for_family(merman_render::DiagramFamilyId::C4)])
}

fn c4_cluster_fill_with_ordinal_palette_theme(fill: CanvasPaint) -> DiagramTheme {
    let styles = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default().with_fill(fill),
            )
            .for_family(merman_render::DiagramFamilyId::C4),
        )
        .with_ordinal_palette(
            ThemeTarget::Cluster,
            OrdinalPalette::new([
                ThemeColorValue::parse("#123456").expect("valid C4 Cluster palette color")
            ])
            .expect("non-empty C4 Cluster palette"),
        );
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile C4 Cluster fill and ordinal palette theme")
}

fn c4_typography_theme(font_stack: FontStack, font_size_px: f32) -> DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_stack(font_stack)
        .with_font_size_px(font_size_px)
        .expect("valid C4 typography");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default()
                    .with_family_style(merman_render::DiagramFamilyId::C4, typography),
            ),
        )
        .expect("compile C4 typography theme")
}

fn try_render_c4_rendered_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed C4 diagram")
        .expect("detect themed C4 diagram");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable C4 session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;

    artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn try_render_c4_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<String> {
    Ok(try_render_c4_rendered_with_theme(source, theme)?
        .svg()
        .to_owned())
}

fn render_c4_svg_with_theme(source: &str, theme: &DiagramTheme) -> String {
    try_render_c4_svg_with_theme(source, theme).expect("render themed C4 SVG")
}

fn try_render_c4_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse C4 resource-bound fixture")
        .expect("detect C4 resource-bound fixture");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin C4 resource-bound session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

fn layout_c4_with_options(source: &str, options: &LayoutOptions) -> C4DiagramLayout {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, options, session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    serde_json::from_value(projection["layout"]["C4Diagram"].clone()).expect("C4 layout projection")
}

fn svg_text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect()
}

fn c4_rect_labeled<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    label: &str,
) -> roxmltree::Node<'a, 'input> {
    let label_node = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
        .unwrap_or_else(|| panic!("missing C4 label {label}"));

    let shape_group = label_node.ancestors().find(|node| {
        node.has_tag_name("g")
            && node
                .attribute("class")
                .is_some_and(|classes| classes.split_whitespace().any(|class| class == "c4-shape"))
    });

    if let Some(group) = shape_group
        && let Some(shape) = group.descendants().find(|node| {
            ["rect", "path", "polygon", "circle", "ellipse"].contains(&node.tag_name().name())
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "label-container")
                })
        })
    {
        return shape;
    }

    label_node
        .ancestors()
        .filter(|node| node.has_tag_name("g"))
        .find_map(|group| {
            group.children().find(|child| {
                child.has_tag_name("rect")
                    && (child.attribute("fill").is_some() || child.attribute("style").is_some())
            })
        })
        .unwrap_or_else(|| panic!("missing C4 shape for label {label}"))
}

fn c4_effective_property(node: roxmltree::Node<'_, '_>, property: &str) -> Option<String> {
    let from_style = node.attribute("style").and_then(|style| {
        style.split(';').find_map(|declaration| {
            let (key, value) = declaration.split_once(':')?;
            (key.trim() == property).then_some(
                value
                    .trim()
                    .trim_end_matches("!important")
                    .trim()
                    .to_owned(),
            )
        })
    });
    from_style.or_else(|| node.attribute(property).map(str::to_owned))
}

#[derive(Debug)]
struct C4TypeWidthProbe;

impl TextMeasurer for C4TypeWidthProbe {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        let width = match text {
            "«person»" => 151.0,
            "«system»" => 252.0,
            "«external_person»" => 399.0,
            _ => 80.0,
        };
        TextMetrics {
            width,
            height: style.font_size.max(1.0),
            line_count: 1,
        }
    }
}

fn deep_c4_boundary_chain(depth: usize) -> String {
    let mut input = String::from("C4Context\n");
    for level in 0..depth {
        input.push_str(&format!("Boundary(b{level}, \"B{level}\") {{\n"));
    }
    input.push_str("System(leaf, \"Leaf\")\n");
    for _ in 0..depth {
        input.push_str("}\n");
    }
    input
}

#[test]
fn c4_cluster_solid_paint_reaches_explicit_solid_and_dashed_boundaries_only() {
    let source = r#"C4Context
Node(solid, "Solid Boundary") {
  System(service, "Service")
}
Boundary(dashed, "Dashed Boundary") {
  System(worker, "Worker")
}
"#;
    let theme = c4_cluster_paint_theme(
        CanvasPaint::solid("#ef4444").expect("valid C4 Cluster fill"),
        CanvasPaint::solid("#2563eb").expect("valid C4 Cluster stroke"),
    );
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");

    for label in ["Solid Boundary", "Dashed Boundary"] {
        let boundary = c4_rect_labeled(&document, label);
        assert_eq!(boundary.attribute("fill"), Some("#ef4444"), "{label}");
        assert_eq!(boundary.attribute("stroke"), Some("#2563eb"), "{label}");
    }

    let ordinary_element = c4_rect_labeled(&document, "Service");
    assert_eq!(
        c4_effective_property(ordinary_element, "fill"),
        Some("#1168BD".to_owned())
    );
    assert_eq!(
        c4_effective_property(ordinary_element, "stroke"),
        Some("#3C7FC0".to_owned())
    );
}

#[test]
fn c4_cluster_transparent_paint_reaches_terminal_boundary_attributes() {
    let source = r#"C4Context
Boundary(boundary, "Boundary") {
  System(service, "Service")
}
"#;
    let theme = c4_cluster_paint_theme(CanvasPaint::Transparent, CanvasPaint::Transparent);
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");

    let boundary = c4_rect_labeled(&document, "Boundary");
    assert_eq!(boundary.attribute("fill"), Some("transparent"));
    assert_eq!(boundary.attribute("stroke"), Some("transparent"));
    let ordinary_element = c4_rect_labeled(&document, "Service");
    assert_eq!(
        c4_effective_property(ordinary_element, "fill"),
        Some("#1168BD".to_owned())
    );
    assert_eq!(
        c4_effective_property(ordinary_element, "stroke"),
        Some("#3C7FC0".to_owned())
    );
}

#[test]
fn c4_cluster_source_paint_outranks_typed_values_per_facet() {
    let source = r##"C4Context
Boundary(fill_owned, "Source Fill") {
  System(fill_child, "Fill Child")
}
Boundary(stroke_owned, "Source Stroke") {
  System(stroke_child, "Stroke Child")
}
UpdateElementStyle(fill_owned, $bgColor="#22c55e")
UpdateElementStyle(stroke_owned, $borderColor="#0f766e")
"##;
    let theme = c4_cluster_paint_theme(
        CanvasPaint::solid("#ef4444").expect("valid C4 Cluster fill"),
        CanvasPaint::solid("#2563eb").expect("valid C4 Cluster stroke"),
    );
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");

    let source_fill = c4_rect_labeled(&document, "Source Fill");
    assert_eq!(source_fill.attribute("fill"), Some("#22c55e"));
    assert_eq!(source_fill.attribute("stroke"), Some("#2563eb"));

    let source_stroke = c4_rect_labeled(&document, "Source Stroke");
    assert_eq!(source_stroke.attribute("fill"), Some("#ef4444"));
    assert_eq!(source_stroke.attribute("stroke"), Some("#0f766e"));
}

#[test]
fn c4_cluster_paint_is_not_applicable_when_source_owns_both_facets() {
    let source = r##"C4Context
Boundary(boundary, "Source Owned") {
  System(service, "Service")
}
UpdateElementStyle(boundary, $bgColor="#22c55e", $borderColor="#0f766e")
"##;
    let theme = c4_cluster_paint_theme(
        CanvasPaint::solid("#ef4444").expect("valid suppressed C4 Cluster fill"),
        CanvasPaint::solid("#2563eb").expect("valid suppressed C4 Cluster stroke"),
    );
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid source-owned C4 SVG XML");
    let boundary = c4_rect_labeled(&document, "Source Owned");

    assert_eq!(boundary.attribute("fill"), Some("#22c55e"));
    assert_eq!(boundary.attribute("stroke"), Some("#0f766e"));
    assert!(!svg.contains("#ef4444"));
    assert!(!svg.contains("#2563eb"));
}

#[test]
fn c4_cluster_ordinal_palette_is_not_applicable_when_typed_fill_wins() {
    let source = r#"C4Context
Boundary(boundary, "Boundary") {
  System(service, "Service")
}
"#;
    let theme = c4_cluster_fill_with_ordinal_palette_theme(
        CanvasPaint::solid("#ef4444").expect("valid C4 Cluster fill"),
    );
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");
    assert_eq!(
        c4_rect_labeled(&document, "Boundary").attribute("fill"),
        Some("#ef4444")
    );
}

#[test]
fn c4_typed_base_typography_reaches_root_css_and_title_evidence() {
    let font_stack = FontStack::single("Inter").expect("valid C4 font stack");
    let theme = c4_typography_theme(font_stack, 18.0);
    let source = "---\ntitle: C4 typography\n---\nC4Context\nSystem(service, \"Service\")\n";

    let rendered =
        try_render_c4_rendered_with_theme(source, &theme).expect("render typed C4 title");
    assert!(rendered.svg().contains("font-family:Inter;"));
    assert!(rendered.svg().contains("font-size:18px;"));
    assert!(rendered.svg().contains(">C4 typography</text>"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn c4_cluster_paint_is_not_applicable_without_explicit_boundaries() {
    let theme = c4_cluster_paint_theme(
        CanvasPaint::solid("#ef4444").expect("valid empty-domain C4 Cluster fill"),
        CanvasPaint::solid("#2563eb").expect("valid empty-domain C4 Cluster stroke"),
    );
    let svg = render_c4_svg_with_theme("C4Context\nSystem(service, \"Service\")\n", &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid empty-domain C4 SVG XML");
    let service = c4_rect_labeled(&document, "Service");

    assert_eq!(
        c4_effective_property(service, "fill"),
        Some("#1168BD".to_owned())
    );
    assert_eq!(
        c4_effective_property(service, "stroke"),
        Some("#3C7FC0".to_owned())
    );
    assert!(!svg.contains("#ef4444"));
    assert!(!svg.contains("#2563eb"));
}

#[test]
fn c4_cluster_paint_rejects_variant_ordinal_clear_gradient_and_pattern_routes() {
    let stops = || {
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#123456").expect("valid C4 gradient start"),
            )
            .expect("valid C4 gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#abcdef").expect("valid C4 gradient end"),
            )
            .expect("valid C4 gradient stop"),
        ]
    };
    let linear = LinearGradient::new(90.0, stops()).expect("valid C4 linear gradient");
    let radial = RadialGradient::new(
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        stops(),
    )
    .expect("valid C4 radial gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#123456").expect("valid C4 pattern color"),
    )
    .expect("valid C4 pattern");
    let mut clear = ThemeStylePatch::default();
    clear.paint.fill = Specified::Clear;
    clear.stroke.paint = Specified::Clear;
    let solid = || {
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid C4 fill"))
            .with_stroke(CanvasPaint::solid("#abcdef").expect("valid C4 stroke"))
    };
    let cases = [
        ThemeRule::new(ThemeTarget::Cluster, solid()).with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::Cluster, solid())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid C4 boundary ordinal")),
        ThemeRule::new(ThemeTarget::Cluster, clear),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::LinearGradient(linear.clone()))
                .with_stroke(CanvasPaint::LinearGradient(linear)),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::RadialGradient(radial.clone()))
                .with_stroke(CanvasPaint::RadialGradient(radial)),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::Pattern(pattern.clone()))
                .with_stroke(CanvasPaint::Pattern(pattern)),
        ),
    ];
    let source =
        "C4Context\nBoundary(boundary, \"Boundary\") {\n  System(service, \"Service\")\n}\n";

    for rule in cases {
        let theme = c4_cluster_rules_theme([rule.for_family(merman_render::DiagramFamilyId::C4)]);
        let error = try_render_c4_svg_with_theme(source, &theme)
            .expect_err("unsupported C4 Cluster paint routes must fail closed");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::C4, 1))
        );
    }
}

#[test]
fn c4_cluster_radius_applies_to_explicit_solid_and_dashed_boundaries_only() {
    let source = r#"C4Context
Node(solid, "Solid Boundary") {
  System(service, "Service")
}
Boundary(dashed, "Dashed Boundary") {
  System(worker, "Worker")
}
"#;
    let theme = c4_cluster_radius_theme(Specified::Value(9.0));
    let svg = render_c4_svg_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid themed C4 SVG XML");

    let solid_boundary = c4_rect_labeled(&document, "Solid Boundary");
    assert_eq!(solid_boundary.attribute("rx"), Some("9"));
    assert_eq!(solid_boundary.attribute("ry"), Some("9"));
    assert_eq!(solid_boundary.attribute("stroke-dasharray"), None);

    let dashed_boundary = c4_rect_labeled(&document, "Dashed Boundary");
    assert_eq!(dashed_boundary.attribute("rx"), Some("9"));
    assert_eq!(dashed_boundary.attribute("ry"), Some("9"));
    assert_eq!(
        dashed_boundary.attribute("stroke-dasharray"),
        Some("7.0,7.0")
    );

    let ordinary_element = c4_rect_labeled(&document, "Service");
    assert_eq!(
        c4_effective_property(ordinary_element, "rx"),
        Some("12px".to_owned())
    );
    assert_eq!(
        c4_effective_property(ordinary_element, "ry"),
        Some("12px".to_owned())
    );

    let cleared_svg = render_c4_svg_with_theme(source, &c4_cluster_radius_theme(Specified::Clear));
    let cleared_document =
        roxmltree::Document::parse(&cleared_svg).expect("valid cleared C4 SVG XML");
    for label in ["Solid Boundary", "Dashed Boundary", "Service"] {
        let rect = c4_rect_labeled(&cleared_document, label);
        let expected_radius = if label == "Service" { "12px" } else { "2.5" };
        assert_eq!(
            c4_effective_property(rect, "rx"),
            Some(expected_radius.to_owned())
        );
        assert_eq!(
            c4_effective_property(rect, "ry"),
            Some(expected_radius.to_owned())
        );
    }
}

#[test]
fn c4_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"C4Context
Person(user, "User")
System_Boundary(system, "Bounded system") {
  Container(api, "API", "Rust")
  ContainerDb(database, "Database", "PostgreSQL")
}
Rel(user, api, "Calls", "HTTPS")
Rel(api, database, "Reads", "SQL")
"#;
    let baseline = try_render_c4_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded C4 baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "C4 fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact C4 SVG byte ceiling");
    let exact = try_render_c4_svg_with_resource_policy(source, exact_policy)
        .expect("the exact C4 family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact C4 SVG byte ceiling");
    let error = try_render_c4_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the C4 family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected C4 MaxSvgBytes rejection, got {error}");
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
fn c4_public_layout_and_svg_render_handle_deep_boundary_chain() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    const DEPTH: usize = 1500;
    let source = deep_c4_boundary_chain(DEPTH);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.metadata().diagram_type, "c4");

    let options = LayoutOptions::default();
    let artifact = family::prepare(parsed, &options, session)
        .expect("layout should not depend on recursive boundary traversal");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    let c4: C4DiagramLayout = serde_json::from_value(projection["layout"]["C4Diagram"].clone())
        .expect("C4 layout projection");

    assert_eq!(c4.boundaries.len(), DEPTH + 1);
    assert_eq!(c4.shapes.len(), 1);
    assert_eq!(c4.shapes[0].alias, "leaf");

    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("SVG painting should use an iterative boundary traversal")
        .svg()
        .to_owned();
    assert!(svg.contains(">Leaf</tspan>"));
    assert!(svg.contains(">B0</tspan>"));
    assert!(svg.contains(&format!(">B{}</tspan>", DEPTH - 1)));
}

#[test]
fn c4_unified_shapes_render_canonical_labels() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
Person(person, "Person")
Container(system, "System", "Rust", "A short description")
Person_Ext(external, "External")
System(framed, "Framed", "A component-shaped system", $shape="component")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");

    for class in ["c4-name", "c4-type", "c4-descr"] {
        assert!(
            document
                .descendants()
                .any(|node| { node.has_tag_name("g") && node.attribute("class") == Some(class) }),
            "missing unified C4 label section {class}: {svg}"
        );
    }
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();
    assert!(type_texts.iter().any(|text| text == "[Person]"));
    assert!(type_texts.iter().any(|text| text == "[Container: Rust]"));
    assert!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-descr"))
            .map(svg_text_content)
            .any(|text| text == "A short description")
    );
    assert_eq!(svg.matches("<circle ").count(), 2);
    assert_eq!(svg.matches("<polygon ").count(), 1);
    assert!(!svg.contains("<<person>>"));
    assert!(!svg.contains("<<system>>"));
    assert!(!svg.contains("<<external_person>>"));
}

#[test]
fn c4_neo_look_reaches_layout_and_svg_terminals() {
    let classic_source = r#"C4Context
ContainerDb(database, "Database", "PostgreSQL")
"#;
    let neo_source = r#"%%{init: {"look": "neo"}}%%
C4Context
ContainerDb(database, "Database", "PostgreSQL")
"#;

    let classic = layout_c4_with_options(classic_source, &LayoutOptions::default());
    let neo = layout_c4_with_options(neo_source, &LayoutOptions::default());
    let classic_shape = classic.shapes.first().expect("classic C4 shape");
    let neo_shape = neo.shapes.first().expect("neo C4 shape");

    // Mermaid's 11.17 cylinder handler increases the vertical label padding from 20px to
    // 24px for Neo. Keep the assertion relational so host text metrics do not become a pixel
    // oracle for this semantic difference.
    assert!(
        neo_shape.height > classic_shape.height + 3.0,
        "Neo cylinder padding must affect layout height: classic={:?}, neo={:?}",
        classic_shape,
        neo_shape
    );

    let svg = render_c4_svg_with_environment(neo_source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid Neo C4 SVG");
    let shape_group = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|c| c == "c4-shape"))
        })
        .expect("Neo C4 shape group");
    assert_eq!(shape_group.attribute("data-look"), Some("neo"));
    let style = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("C4 stylesheet");
    assert!(
        style.contains(r#"[data-look="neo"].node path"#),
        "the common Neo selector must be reachable from C4 terminals"
    );
}

#[test]
fn c4_neo_component_uses_framed_rectangle_geometry() {
    let source = r#"%%{init: {"look": "neo", "c4": {"width": 1}}}%%
C4Context
System(framed, "Framed", "Component description", $shape="component")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::default());
    let shape = layout.shapes.first().expect("Neo component shape");

    let mut content_width = shape.label.width.max(shape.type_block.width);
    let mut content_height = shape.label.height + shape.type_block.height;
    let mut section_count = 2_usize;
    if let Some(description) = shape.descr.as_ref() {
        content_width = content_width.max(description.width);
        content_height += description.height;
        section_count += 1;
    }
    content_height += 3.0 * section_count.saturating_sub(1) as f64;

    // Mermaid maps C4 `component` to `fr-rect`/subroutine. Its Neo geometry reserves two
    // eight-pixel frames plus 28px horizontal label padding and 12px total vertical padding.
    assert!((shape.width - (content_width + 44.0)).abs() < 1e-6);
    assert!((shape.height - (content_height + 12.0)).abs() < 1e-6);

    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid Neo component SVG");
    let shape_group = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-framed"))
        .expect("Neo component group");
    let label_group = shape_group
        .children()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
        .expect("Neo component label group");
    let transform = label_group
        .attribute("transform")
        .and_then(|value| value.strip_prefix("translate("))
        .and_then(|value| value.strip_suffix(')'))
        .expect("translate transform");
    let mut values = transform.split(',').map(|value| {
        value
            .trim()
            .parse::<f64>()
            .expect("numeric translate component")
    });
    let translate_x = values.next().expect("translate x");
    let translate_y = values.next().expect("translate y");
    assert!(values.next().is_none());
    assert!((translate_x + content_width / 2.0).abs() < 1e-6);
    assert!((translate_y + content_height / 2.0).abs() < 1e-6);
}

#[test]
fn c4_hand_drawn_look_is_rejected_instead_of_emitting_classic_geometry() {
    let source = r#"%%{init: {"look": "handDrawn"}}%%
C4Context
System(service, "Service")
"#;

    let error = try_render_c4_svg_with_environment(source, &RenderEnvironment::deterministic())
        .expect_err("typed C4 must not silently downgrade handDrawn to classic");
    assert!(matches!(
        error,
        merman_render::Error::InvalidModel { message }
            if message.contains("look `handDrawn` is not supported")
    ));
}

#[test]
fn c4_shape_named_type_does_not_override_the_rendered_stereotype() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
Container(app, "Application", $type="Domain Service")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();

    assert_eq!(type_texts, ["[Container]"]);
}

#[test]
fn c4_shape_technology_reaches_layout_and_svg_while_named_type_is_ignored() {
    let source = r#"C4Context
Container(app, "Application", "Rust", $type="Domain Service")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::default());
    let shape = layout
        .shapes
        .iter()
        .find(|shape| shape.alias == "app")
        .expect("C4 application shape");

    assert_eq!(shape.type_block.text, "[Container: Rust]");

    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();

    assert_eq!(type_texts, ["[Container: Rust]"]);
    assert!(!svg.contains("Domain Service"));
}

#[test]
fn c4_uses_explicit_screen_available_width_without_changing_container_geometry() {
    let source = include_str!(
        "../../../fixtures/c4/upstream_docs_c4_c4_container_diagram_c4container_006.mmd"
    );
    let default = layout_c4_with_options(source, &LayoutOptions::default());
    let wide_screen = layout_c4_with_options(
        source,
        &LayoutOptions::default().with_screen_available_width(1280.0),
    );

    assert_eq!(default.container_width, 800.0);
    assert_eq!(default.screen_available_width, None);
    assert_eq!(wide_screen.container_width, 800.0);
    assert_eq!(wide_screen.screen_available_width, Some(1280.0));
    assert!(wide_screen.width > default.width);
    assert!(wide_screen.height < default.height);
}

#[test]
fn c4_svg_paints_each_boundary_subtree_in_mermaid_order() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
System(root_before, "Root Before")
Boundary(outer, "Outer Boundary") {
  System(outer_before, "Outer Before")
  Boundary(inner, "Inner Boundary") {
    System(inner_shape, "Inner Shape")
  }
  System(outer_after, "Outer After")
}
System(root_after, "Root After")
Rel(inner_shape, root_before, "Leaves subtree")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let labels = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(svg_text_content)
        .filter(|text| {
            matches!(
                text.as_str(),
                "Root Before"
                    | "Root After"
                    | "Outer Before"
                    | "Outer After"
                    | "Inner Shape"
                    | "Inner Boundary"
                    | "Outer Boundary"
                    | "Leaves subtree"
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        [
            "Root Before",
            "Root After",
            "Outer Before",
            "Outer After",
            "Inner Shape",
            "Inner Boundary",
            "Outer Boundary",
            "Leaves subtree",
        ]
    );
}

#[test]
fn c4_relation_keeps_explicit_line_and_text_styles() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
System(a, "A")
System(b, "B")
Rel(a, b, "Calls", "HTTPS")
UpdateRelStyle(a, b, $textColor="red", $lineColor="blue", $offsetX="10", $offsetY="20")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let calls = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "Calls")
        .expect("relationship label");
    let technology = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "[HTTPS]")
        .expect("relationship technology label");
    let line = document
        .descendants()
        .find(|node| {
            matches!(node.tag_name().name(), "line" | "path")
                && node.attribute("stroke") == Some("blue")
        })
        .expect("relationship line");

    assert_eq!(calls.attribute("fill"), Some("red"));
    assert!(calls.attribute("style").is_some_and(|style| {
        style.contains("text-anchor: middle") && style.contains("font-family:")
    }));
    assert_eq!(technology.attribute("fill"), Some("red"));
    assert_eq!(technology.attribute("font-style"), Some("italic"));
    assert_eq!(line.attribute("stroke-width"), Some("1"));
    assert_eq!(line.attribute("style"), Some("fill: none;"));
}
