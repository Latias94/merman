use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, InsetsPx,
    MermaidThemeCompatibility, OrdinalPalette, OrdinalSelector, Specified, ThemeColorValue,
    ThemeGeometryPatch, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::KanbanDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn kanban_task_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Kanban task theme")
}

fn kanban_task_radius_style(radius: Specified<f32>) -> ThemeStylePatch {
    ThemeStylePatch {
        geometry: ThemeGeometryPatch { radius },
        ..ThemeStylePatch::default()
    }
}

fn kanban_task_radius_theme(radius: Specified<f32>) -> DiagramTheme {
    kanban_task_rules_theme([ThemeRule::new(
        ThemeTarget::Task,
        kanban_task_radius_style(radius),
    )])
}

fn kanban_task_fill_theme(paint: CanvasPaint) -> DiagramTheme {
    kanban_task_rules_theme([ThemeRule::new(
        ThemeTarget::Task,
        ThemeStylePatch::default().with_fill(paint),
    )])
}

fn kanban_task_stroke_theme(paint: CanvasPaint) -> DiagramTheme {
    kanban_task_rules_theme([ThemeRule::new(
        ThemeTarget::Task,
        ThemeStylePatch::default().with_stroke(paint),
    )])
}

fn kanban_task_palette_styles(colors: &[&str]) -> ThemeRuleSet {
    let palette = OrdinalPalette::new(
        colors
            .iter()
            .map(|color| ThemeColorValue::parse(color).expect("valid Kanban task palette color")),
    )
    .expect("non-empty Kanban task palette");
    ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Task, palette)
}

fn kanban_task_palette_theme(colors: &[&str]) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(kanban_task_palette_styles(colors)))
        .expect("compile Kanban task palette")
}

fn kanban_task_label_rule(color: &str, ordinal: usize) -> ThemeRule {
    ThemeRule::new(
        ThemeTarget::TaskLabel,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid(color).expect("valid Kanban task-label foreground")),
    )
    .for_family(merman_render::DiagramFamilyId::KANBAN)
    .with_ordinal(OrdinalSelector::exact(ordinal).expect("valid Kanban task-label ordinal"))
}

fn kanban_task_label_theme(color: &str) -> DiagramTheme {
    kanban_task_rules_theme([kanban_task_label_rule(color, 1)])
}

fn kanban_typography_theme(style: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default()
                    .with_family_style(merman_render::DiagramFamilyId::KANBAN, style),
            ),
        )
        .expect("compile Kanban typography theme")
}

fn render_kanban_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
) -> family::RenderedFamilySvg {
    try_render_kanban_with_theme_and_engine(source, theme, engine, portability)
        .expect("render themed Kanban")
}

fn try_render_kanban_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_kanban_with_theme_engine_and_environment(
        source,
        theme,
        engine,
        portability,
        RenderEnvironment::deterministic(),
    )
}

fn try_render_kanban_with_layout_and_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> merman_render::Result<(KanbanDiagramLayout, family::RenderedFamilySvg)> {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Kanban")
        .expect("detect themed Kanban")
        .into_presentation();
    let session = environment
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Kanban session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let projection = artifact.layout_json()?;
    let layout = serde_json::from_value(projection["layout"]["KanbanDiagram"].clone())
        .expect("Kanban layout projection");
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("kanban-palette".to_string()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok((layout, rendered))
}

fn try_render_kanban_with_theme_engine_and_environment(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
    environment: RenderEnvironment,
) -> merman_render::Result<family::RenderedFamilySvg> {
    try_render_kanban_with_layout_and_environment(source, theme, engine, portability, environment)
        .map(|(_, rendered)| rendered)
}

fn kanban_task_rect_style<'input>(
    document: &'input roxmltree::Document<'input>,
    id: &str,
) -> &'input str {
    document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(id))
        .and_then(|group| group.children().find(|node| node.has_tag_name("rect")))
        .and_then(|rect| rect.attribute("style"))
        .expect("Kanban task rect style")
}

fn kanban_task_label_style<'input>(
    document: &'input roxmltree::Document<'input>,
    id: &str,
) -> &'input str {
    document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(id))
        .and_then(|group| {
            group.descendants().find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|token| token == "label")
                    })
            })
        })
        .and_then(|label| label.attribute("style"))
        .expect("Kanban task label style")
}

fn kanban_task_title_div_class<'input>(
    document: &'input roxmltree::Document<'input>,
    id: &str,
) -> Option<&'input str> {
    document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(id))
        .and_then(|group| {
            group
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
        })
        .and_then(|foreign_object| {
            foreign_object
                .children()
                .find(|node| node.has_tag_name("div"))
        })
        .and_then(|div| div.attribute("class"))
}

fn kanban_task_rect_height(document: &roxmltree::Document<'_>, id: &str) -> f64 {
    document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(id))
        .and_then(|group| group.children().find(|node| node.has_tag_name("rect")))
        .and_then(|rect| rect.attribute("height"))
        .and_then(|height| height.parse::<f64>().ok())
        .expect("Kanban task rect height")
}

fn kanban_root_font_size_css(svg: &str, diagram_id: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Kanban SVG");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Kanban stylesheet");
    css.split('}')
        .find(|rule| rule.starts_with(&format!("#{diagram_id}{{")))
        .expect("Kanban root CSS rule")
        .split(';')
        .find_map(|declaration| declaration.strip_prefix("font-size:"))
        .expect("Kanban root font size")
        .to_owned()
}

fn kanban_task_label_foreign_object(
    document: &roxmltree::Document<'_>,
    id: &str,
    text: &str,
) -> (f64, f64) {
    let task = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(id))
        .expect("Kanban task group");
    let paragraph = task
        .descendants()
        .find(|node| node.has_tag_name("p") && node.text() == Some(text))
        .unwrap_or_else(|| panic!("Kanban task label {text:?}"));
    let foreign_object = paragraph
        .ancestors()
        .find(|node| node.has_tag_name("foreignObject"))
        .expect("Kanban label foreignObject");
    (
        foreign_object
            .attribute("width")
            .expect("Kanban label foreignObject width")
            .parse()
            .expect("numeric Kanban label width"),
        foreign_object
            .attribute("height")
            .expect("Kanban label foreignObject height")
            .parse()
            .expect("numeric Kanban label height"),
    )
}

#[derive(Debug)]
struct KanbanLayoutProbeMeasurer {
    expected_font_size: f64,
    expected_size_calls: Arc<AtomicUsize>,
    unexpected_size_calls: Arc<AtomicUsize>,
}

impl KanbanLayoutProbeMeasurer {
    fn metrics(text: &str, font_size: f64) -> (f64, f64) {
        let height = match text {
            "Todo" => font_size + 8.0,
            "Task" => font_size + 13.0,
            "MC-2038" => font_size - 6.0,
            "Alice" => font_size + 10.0,
            "Next" => font_size + 4.0,
            _ => font_size,
        };
        let width = match text {
            "Todo" => 64.0,
            "Task" => 72.0,
            "MC-2038" => 60.0,
            "Alice" => 50.0,
            "Next" => 48.0,
            _ => (text.chars().count() as f64 * 8.0).max(1.0),
        };
        (width, height.max(1.0))
    }

    fn record_font_size(&self, style: &TextStyle) {
        if (style.font_size - self.expected_font_size).abs() < f64::EPSILON {
            self.expected_size_calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.unexpected_size_calls.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl TextMeasurer for KanbanLayoutProbeMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.record_font_size(style);
        let (width, height) = Self::metrics(text, style.font_size);
        TextMetrics {
            width,
            height,
            line_count: 1,
        }
    }
}

fn kanban_probe_environment(
    expected_font_size: f64,
    profile_id: &str,
) -> (RenderEnvironment, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let expected_size_calls = Arc::new(AtomicUsize::new(0));
    let unexpected_size_calls = Arc::new(AtomicUsize::new(0));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new(profile_id).expect("valid Kanban probe profile id"),
        "test",
    )
    .expect("valid Kanban probe profile identity");
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(KanbanLayoutProbeMeasurer {
                expected_font_size,
                expected_size_calls: Arc::clone(&expected_size_calls),
                unexpected_size_calls: Arc::clone(&unexpected_size_calls),
            }),
        )),
    );
    (environment, expected_size_calls, unexpected_size_calls)
}

fn render_kanban_artifact(
    artifact: family::FamilyRenderArtifact,
) -> merman_render::Result<(KanbanDiagramLayout, String)> {
    let projection = artifact.layout_json()?;
    let layout = serde_json::from_value(projection["layout"]["KanbanDiagram"].clone())
        .expect("Kanban layout projection");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("kanban-markdown".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )?
        .svg()
        .to_owned();
    Ok((layout, svg))
}

fn try_render_kanban_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> merman_render::Result<(KanbanDiagramLayout, String)> {
    let parsed = common::install_parse_compatibility(theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Kanban")
        .expect("detect themed Kanban")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Kanban session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    render_kanban_artifact(artifact)
}

fn render_kanban_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> (KanbanDiagramLayout, String) {
    try_render_kanban_svg_with_theme(source, theme).expect("render themed Kanban SVG")
}

fn parse_layout_and_render(source: &str) -> (KanbanDiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Kanban")
        .expect("detect Kanban")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("start deterministic render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare Kanban layout");
    render_kanban_artifact(artifact).expect("render Kanban SVG")
}

fn try_render_kanban_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse resource-bound Kanban")
        .expect("detect resource-bound Kanban")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("start resource-bound Kanban render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("kanban-bounded".to_string()),
            ..Default::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn kanban_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"kanban
  todo[Todo]
    task[Bounded output task]@{ ticket: MC-2038, assigned: 'Alice', priority: 'High' }
  done[Done]
    shipped[Shipped]
"#;
    let baseline = try_render_kanban_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Kanban baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Kanban fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Kanban SVG byte ceiling");
    let exact = try_render_kanban_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Kanban family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Kanban SVG byte ceiling");
    let error = try_render_kanban_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Kanban family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Kanban MaxSvgBytes rejection, got {error}");
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
fn kanban_markdown_metrics_drive_canonical_layout_and_svg() {
    let (markdown_layout, markdown_svg) = parse_layout_and_render(
        "kanban\n  todo[Todo]\n    task[*aaaa aaaa aaaaaaa*]\n    next[Next]\n",
    );
    let (plain_layout, _) = parse_layout_and_render(
        "kanban\n  todo[Todo]\n    task[aaaa aaaa aaaaaaa]\n    next[Next]\n",
    );

    assert_eq!(
        markdown_layout.items[0].height,
        plain_layout.items[0].height
    );
    assert_eq!(
        markdown_layout.items[1].center_y,
        plain_layout.items[1].center_y
    );
    assert_eq!(
        markdown_layout.sections[0].rect_height,
        plain_layout.sections[0].rect_height
    );
    assert!(
        markdown_svg.contains("<p><em>aaaa aaaa aaaaaaa</em></p>"),
        "{markdown_svg}"
    );
}

#[test]
fn kanban_static_task_radius_reaches_layout_priority_geometry_svg_and_evidence() {
    let source = concat!(
        "kanban\n",
        "  todo[Todo]\n",
        "    task[Radius task]@{ priority: 'High' }\n",
    );
    let (_, baseline) = parse_layout_and_render(source);
    let empty_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty typed theme");
    let (_, empty_theme_svg) = render_kanban_svg_with_theme(source, &empty_theme);
    assert_eq!(
        empty_theme_svg.as_bytes(),
        baseline.as_bytes(),
        "an absent Kanban task radius must preserve default SVG bytes"
    );

    let (layout, svg) =
        render_kanban_svg_with_theme(source, &kanban_task_radius_theme(Specified::Value(8.0)));
    let item = layout.items.first().expect("themed Kanban item");
    assert_eq!(item.rx, 8.0);
    assert_eq!(item.ry, 8.0);
    assert!(layout.sections.iter().all(|section| section.rx == 5.0));

    let document = roxmltree::Document::parse(&svg).expect("valid themed Kanban SVG XML");
    let item_group = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some("kanban-markdown-task"))
        .expect("themed Kanban item group");
    let item_rect = item_group
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("themed Kanban item rect");
    assert_eq!(item_rect.attribute("rx"), Some("8"));
    assert_eq!(item_rect.attribute("ry"), Some("8"));

    let priority_line = item_group
        .children()
        .find(|node| node.has_tag_name("line"))
        .expect("Kanban priority line");
    let y1 = priority_line
        .attribute("y1")
        .expect("priority line y1")
        .parse::<f64>()
        .expect("numeric priority line y1");
    let y2 = priority_line
        .attribute("y2")
        .expect("priority line y2")
        .parse::<f64>()
        .expect("numeric priority line y2");
    assert_eq!(y1, -item.height / 2.0 + (item.rx / 2.0).floor());
    assert_eq!(
        y2,
        -item.height / 2.0 + item.height - (item.rx / 2.0).floor()
    );
}

#[test]
fn kanban_static_task_fill_reaches_each_terminal_rect_without_palette_fallback() {
    let source = concat!(
        "kanban\n",
        "  todo[Todo]\n",
        "    first[First]\n",
        "    second[Second]\n",
    );

    for (paint, expected_fill) in [
        (
            CanvasPaint::solid("#123456").expect("valid Kanban task fill"),
            "fill:#123456;",
        ),
        (CanvasPaint::Transparent, "fill:transparent;"),
    ] {
        let theme = kanban_task_fill_theme(paint);
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Kanban fill SVG");
        assert_eq!(
            kanban_task_rect_style(&document, "kanban-palette-first"),
            expected_fill
        );
        assert_eq!(
            kanban_task_rect_style(&document, "kanban-palette-second"),
            expected_fill
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn kanban_static_task_fill_respects_the_mermaid_background_owner() {
    let theme =
        kanban_task_fill_theme(CanvasPaint::solid("#123456").expect("valid Kanban task fill"));
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "background": "#fedcba" }
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Kanban background-owned SVG");
    assert!(!kanban_task_rect_style(&document, "kanban-palette-task").contains("#123456"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_static_task_stroke_reaches_each_terminal_rect() {
    let source = concat!(
        "kanban\n",
        "  todo[Todo]\n",
        "    first[First]\n",
        "    second[Second]\n",
    );

    for (paint, expected_stroke) in [
        (
            CanvasPaint::solid("#654321").expect("valid Kanban task stroke"),
            "stroke:#654321;",
        ),
        (CanvasPaint::Transparent, "stroke:transparent;"),
    ] {
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &kanban_task_stroke_theme(paint),
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Kanban stroke SVG");
        assert_eq!(
            kanban_task_rect_style(&document, "kanban-palette-first"),
            expected_stroke
        );
        assert_eq!(
            kanban_task_rect_style(&document, "kanban-palette-second"),
            expected_stroke
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn kanban_static_task_stroke_respects_the_mermaid_node_border_owner() {
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &kanban_task_stroke_theme(CanvasPaint::solid("#654321").expect("valid Kanban task stroke")),
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "nodeBorder": "#fedcba" }
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Kanban nodeBorder-owned SVG");
    assert_eq!(
        kanban_task_rect_style(&document, "kanban-palette-task"),
        "",
        "source-owned nodeBorder must leave the terminal without typed inline stroke"
    );
    assert!(rendered.svg().contains("stroke:#fedcba;"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_task_palette_reaches_terminal_paint_and_wraps_the_twelve_mermaid_slots() {
    let theme = kanban_task_palette_theme(&[
        "#123456",
        "transparent",
        "#111111",
        "#222222",
        "#333333",
        "#444444",
        "#555555",
        "#666666",
        "#777777",
        "#888888",
        "#999999",
        "#aaaaaa",
        "#abcdef",
    ]);
    let rendered = render_kanban_with_theme_and_engine(
        concat!(
            "kanban\n  todo[Todo]\n",
            "    first[First]\n",
            "    second[Second]\n",
            "    third[Third]\n",
            "    fourth[Fourth]\n",
            "    fifth[Fifth]\n",
            "    sixth[Sixth]\n",
            "    seventh[Seventh]\n",
            "    eighth[Eighth]\n",
            "    ninth[Ninth]\n",
            "    tenth[Tenth]\n",
            "    eleventh[Eleventh]\n",
            "    twelfth[Twelfth]\n",
            "    thirteenth[Thirteenth]\n",
        ),
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Kanban palette SVG");

    assert_eq!(
        kanban_task_rect_style(&document, "kanban-palette-first"),
        "fill:hsl(210, 65.3846153846%, 30.3921568627%);"
    );
    assert_eq!(
        kanban_task_rect_style(&document, "kanban-palette-second"),
        "fill:hsla(0, 0%, 10%, 0);"
    );
    assert_eq!(
        kanban_task_rect_style(&document, "kanban-palette-thirteenth"),
        "fill:hsl(210, 65.3846153846%, 30.3921568627%);",
        "the terminal palette must preserve Mermaid's twelve-slot ring"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_task_palette_derives_dark_mode_from_the_final_typed_winner() {
    let theme = kanban_task_palette_theme(&["#123456"]);
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "darkMode": true
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid dark Kanban SVG");

    assert_eq!(
        kanban_task_rect_style(&document, "kanban-palette-task"),
        "fill:hsl(210, 65.3846153846%, 10.3921568627%);"
    );
}

#[test]
fn kanban_theme_variable_dark_mode_reaches_sections_and_task_palettes() {
    let theme = kanban_task_palette_theme(&["#123456"]);
    let source = "kanban\n  todo[Todo]\n    task[Task]\n";
    for (root, variable, dark) in [
        (None, serde_json::json!(true), true),
        (Some(false), serde_json::json!(true), true),
        (Some(true), serde_json::json!(false), false),
        (None, serde_json::json!("false"), true),
        (None, serde_json::json!(0), false),
    ] {
        let mut config = serde_json::json!({
            "themeVariables": { "darkMode": variable, "cScale1": "#123456" }
        });
        if let Some(root) = root {
            config["darkMode"] = root.into();
        }
        let fill = if dark {
            "hsl(210, 65.3846153846%, 10.3921568627%)"
        } else {
            "hsl(210, 65.3846153846%, 30.3921568627%)"
        };
        for from_source in [false, true] {
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{source}"),
                    serde_json::json!({}),
                )
            } else {
                (source.to_owned(), config.clone())
            };
            let rendered = render_kanban_with_theme_and_engine(
                &source,
                &theme,
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
                ThemePortabilityRequirement::RequirePortable,
            );
            let document = roxmltree::Document::parse(rendered.svg()).expect("valid Kanban SVG");
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .expect("Kanban stylesheet");
            assert!(
                css.contains(&format!(".section-0 path{{fill:{fill};")),
                "section dark mode: config={config}, source={from_source}"
            );
            assert_eq!(
                kanban_task_rect_style(&document, "kanban-palette-task"),
                format!("fill:{fill};"),
                "task palette dark mode: config={config}, source={from_source}"
            );
        }
    }
}

#[test]
fn kanban_selected_dark_palette_keeps_its_independent_dark_mode_input() {
    let theme = kanban_task_palette_theme(&["#123456"]);
    // Upstream Dark selects a palette; it does not initialize themeVariables.darkMode=true.
    for dark in [false, true] {
        let mut config = serde_json::json!({
            "theme": "dark", "themeVariables": { "cScale1": "#123456" }
        });
        if dark {
            config["themeVariables"]["darkMode"] = true.into();
        }
        let rendered = render_kanban_with_theme_and_engine(
            "kanban\n  todo[Todo]\n    task[Task]\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid dark Kanban SVG");
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Kanban stylesheet");
        let fill = if dark {
            "hsl(210, 65.3846153846%, 10.3921568627%)"
        } else {
            "hsl(210, 65.3846153846%, 30.3921568627%)"
        };
        assert!(css.contains(&format!(".section-0 path{{fill:{fill};")));
        assert_eq!(
            kanban_task_rect_style(&document, "kanban-palette-task"),
            format!("fill:{fill};")
        );
    }
}

#[test]
fn kanban_task_label_foreground_survives_the_resvg_fallback_without_a_palette() {
    let theme = kanban_task_label_theme("#123456");
    let rendered = render_kanban_with_theme_and_engine(
        concat!(
            "kanban\n  todo[Todo]\n",
            "    task[Task]@{ ticket: MC-2038, assigned: 'Alice' }\n",
        ),
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let native = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("finalize Kanban task-label foreground for resvg");
    let native_svg = native.svg().as_str();
    let document = roxmltree::Document::parse(native_svg).expect("valid native Kanban SVG");
    for expected in ["Task", "MC-2038", "Alice"] {
        let task_text = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some(expected))
            .unwrap_or_else(|| panic!("native Kanban {expected} label"));
        assert_eq!(task_text.attribute("fill"), Some("#123456"));
    }
    assert!(
        native_svg.contains(r#"data-merman-foreignobject="fallback""#),
        "the native projection must prove the label foreground after foreignObject fallback"
    );
}

#[test]
fn kanban_task_palette_respects_the_actual_mermaid_card_fill_owner() {
    let theme = kanban_task_palette_theme(&["#123456"]);
    let site = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "background": "#fedcba" }
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
    );
    let site_document = roxmltree::Document::parse(site.svg()).expect("valid site-owned Kanban");
    assert_eq!(
        kanban_task_rect_style(&site_document, "kanban-palette-task"),
        ""
    );
    assert!(
        site.svg().contains("#fedcba"),
        "the site-owned background must remain in Mermaid's task-card CSS"
    );

    let source = render_kanban_with_theme_and_engine(
        concat!(
            "%%{init: {\"themeVariables\": {\"background\": \"#fedcba\"}}}%%\n",
            "kanban\n  todo[Todo]\n    task[Task]\n",
        ),
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "secure": []
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
    );
    let source_document =
        roxmltree::Document::parse(source.svg()).expect("valid source-owned Kanban");
    assert_eq!(
        kanban_task_rect_style(&source_document, "kanban-palette-task"),
        ""
    );
    assert!(
        source.svg().contains("#fedcba"),
        "the source-owned background must remain in Mermaid's task-card CSS"
    );

    for (name, engine, source) in [
        (
            "site",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "textColor": "#fedcba" }
                    }),
                ),
            ),
            "kanban\n  todo[Todo]\n    task[Task]\n",
        ),
        (
            "source",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "secure": []
                    }),
                ),
            ),
            concat!(
                "%%{init: {\"themeVariables\": {\"textColor\": \"#fedcba\"}}}%%\n",
                "kanban\n  todo[Todo]\n    task[Task]\n",
            ),
        ),
    ] {
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            engine,
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|_| panic!("valid {name}-owned Kanban SVG"));
        assert_eq!(
            kanban_task_rect_style(&document, "kanban-palette-task"),
            "fill:hsl(210, 65.3846153846%, 30.3921568627%);",
            "{name}-owned text must not suppress the independently owned task palette"
        );
        assert_eq!(
            kanban_task_label_style(&document, "kanban-palette-task"),
            "text-align:left !important",
            "{name}-owned text must retain the compatibility foreground"
        );
        assert!(
            rendered.svg().contains("#fedcba"),
            "the {name}-owned text color must remain in Kanban CSS"
        );
    }

    let compatibility = MermaidThemeCompatibility::default()
        .with_variable("background", "#fedcba")
        .expect("valid explicit Kanban background compatibility")
        .with_variable("textColor", "#abcdef")
        .expect("valid explicit Kanban text compatibility");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_styles(kanban_task_palette_styles(&["#123456"]))
                .with_mermaid_compatibility(compatibility),
        )
        .expect("compile explicit Mermaid Kanban palette fixture");
    let spec = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let spec_document = roxmltree::Document::parse(spec.svg()).expect("valid spec-owned Kanban");
    assert_eq!(
        kanban_task_rect_style(&spec_document, "kanban-palette-task"),
        ""
    );
    assert_eq!(
        kanban_task_label_style(&spec_document, "kanban-palette-task"),
        "text-align:left !important"
    );
    assert!(
        spec.svg().contains("#fedcba") && spec.svg().contains("#abcdef"),
        "the spec-owned background and text must remain in Mermaid's task-card CSS"
    );

    for (site_config, expected_surface) in [
        (
            serde_json::json!({ "themeVariables": { "cScale0": "#fedcba" } }),
            "section",
        ),
        (
            serde_json::json!({ "themeVariables": { "git0": "#fedcba" } }),
            "root",
        ),
    ] {
        let theme = kanban_task_palette_theme(&["#123456"]);
        let rendered = render_kanban_with_theme_and_engine(
            "kanban\n  todo[Todo]\n    task[Task]\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(site_config),
            ),
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg())
            .expect("valid independently owned Kanban surface");
        assert_eq!(
            kanban_task_rect_style(&document, "kanban-palette-task"),
            "fill:hsl(210, 65.3846153846%, 30.3921568627%);"
        );
        assert!(
            rendered.svg().contains("#fedcba"),
            "the explicit {expected_surface} color must remain in Mermaid CSS"
        );
    }
}

#[test]
fn kanban_task_label_foreground_respects_only_the_terminal_text_color_owner() {
    let theme = kanban_task_label_theme("#123456");
    let plain_source = "kanban\n  todo[Todo]\n    task[Task]\n";

    for (name, engine, source) in [
        (
            "site background",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "background": "#fedcba" }
                    }),
                ),
            ),
            plain_source,
        ),
        (
            "source background",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "secure": []
                    }),
                ),
            ),
            concat!(
                "%%{init: {\"themeVariables\": {\"background\": \"#fedcba\"}}}%%\n",
                "kanban\n  todo[Todo]\n    task[Task]\n",
            ),
        ),
        (
            "site primaryTextColor",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "primaryTextColor": "#fedcba" }
                    }),
                ),
            ),
            plain_source,
        ),
        (
            "source primaryTextColor",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "secure": []
                    }),
                ),
            ),
            concat!(
                "%%{init: {\"themeVariables\": {\"primaryTextColor\": \"#fedcba\"}}}%%\n",
                "kanban\n  todo[Todo]\n    task[Task]\n",
            ),
        ),
    ] {
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            engine,
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|_| panic!("valid {name} Kanban SVG"));
        assert_eq!(
            kanban_task_rect_style(&document, "kanban-palette-task"),
            "",
            "{name} must not manufacture a typed card palette"
        );
        assert_eq!(
            kanban_task_label_style(&document, "kanban-palette-task"),
            "color:#123456;fill:#123456;text-align:left !important",
            "{name} is not the terminal Kanban label owner"
        );
    }

    for (name, engine, source) in [
        (
            "site textColor",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "textColor": "#fedcba" }
                    }),
                ),
            ),
            plain_source,
        ),
        (
            "source textColor",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "secure": []
                    }),
                ),
            ),
            concat!(
                "%%{init: {\"themeVariables\": {\"textColor\": \"#fedcba\"}}}%%\n",
                "kanban\n  todo[Todo]\n    task[Task]\n",
            ),
        ),
    ] {
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            engine,
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|_| panic!("valid {name} Kanban SVG"));
        assert_eq!(
            kanban_task_label_style(&document, "kanban-palette-task"),
            "text-align:left !important",
            "{name} must suppress only the typed label route"
        );
        assert!(
            rendered.svg().contains("#fedcba"),
            "{name} must remain in the compatibility stylesheet"
        );
    }
}

#[test]
fn kanban_task_label_inline_html_color_owns_its_visible_text_run() {
    let theme = kanban_task_label_theme("#123456");
    let rendered = render_kanban_with_theme_and_engine(
        concat!(
            "%%{init: {\"securityLevel\": \"loose\"}}%%\n",
            "kanban\n  todo[Todo]\n",
            "    task[\"<span style='color:transparent'>Hidden</span>\"]\n",
        ),
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid source-owned Kanban label SVG");

    assert_eq!(
        kanban_task_label_style(&document, "kanban-palette-task"),
        "text-align:left !important",
        "an inline descendant color owns the only visible run, so the typed foreground is N/A"
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_mixed_color_owners_cannot_prove_portable_task_label_fill() {
    let theme = kanban_task_label_theme("#123456");
    let source = concat!(
        "%%{init: {\"securityLevel\": \"loose\"}}%%\n",
        "kanban\n  todo[Todo]\n",
        "    task[\"Visible <span style='color:transparent'>Hidden</span>\"]\n",
    );
    let rendered = render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid mixed-owner Kanban label SVG");

    assert_eq!(
        kanban_task_label_style(&document, "kanban-palette-task"),
        "color:#123456;fill:#123456;text-align:left !important"
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("span")
                && node.attribute("style").is_some_and(|style| {
                    style.split(';').any(|declaration| {
                        declaration
                            .split_once(':')
                            .is_some_and(|(property, value)| {
                                property.trim().eq_ignore_ascii_case("color")
                                    && value.trim().eq_ignore_ascii_case("transparent")
                            })
                    })
                })
        }),
        "the source-owned descendant run must remain intact"
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);

    let error = match try_render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    ) {
        Ok(_) => panic!("mixed XHTML color ownership must fail closed in portable mode"),
        Err(error) => error,
    };
    assert!(
        matches!(
            error,
            merman_render::Error::UnverifiedFamilyTheme {
                family_id: merman_render::DiagramFamilyId::KANBAN,
                residual_count: 1,
            }
        ),
        "unexpected Kanban mixed-color portability error: {error:?}"
    );
}

#[test]
fn kanban_task_label_css_class_ownership_is_not_portably_provable() {
    let theme = kanban_task_label_theme("#123456");
    let source = concat!(
        "%%{init: {\"securityLevel\": \"loose\"}}%%\n",
        "kanban\n  todo[Todo]\n",
        "    task[\"Visible <span class='label'>CSS-owned</span>\"]\n",
    );
    let rendered = render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);

    let error = match try_render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    ) {
        Ok(_) => panic!("class-owned XHTML color must fail closed in portable mode"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        merman_render::Error::UnverifiedFamilyTheme {
            family_id: merman_render::DiagramFamilyId::KANBAN,
            residual_count: 1,
        }
    ));
}

#[test]
fn kanban_task_label_unknown_metadata_is_residual_despite_a_verified_title() {
    let theme = kanban_task_label_theme("#123456");
    for base_size in [0.0, 0.5] {
        let title = if base_size == 0.0 {
            "<span style='font-size:12px'>Task</span>"
        } else {
            "Task"
        };
        for field in ["ticket", "assigned"] {
            for metadata in [
                "Visible <span class='host'>Unknown</span>",
                "Visible <span style='font-size:var(--host-size)'>Unknown</span>",
            ] {
                let source = format!(
                    "kanban\n  todo[Todo]\n    task[\"{title}\"]@{{ {field}: \"{metadata}\" }}\n"
                );
                let engine = || {
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({
                                "securityLevel": "loose",
                                "themeVariables": { "fontSize": format!("{base_size}px") },
                                "kanban": { "ticketBaseUrl": "https://example.invalid/#TICKET#" }
                            }),
                        ),
                    )
                };
                let rendered = render_kanban_with_theme_and_engine(
                    &source,
                    &theme,
                    engine(),
                    ThemePortabilityRequirement::BestEffort,
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert_eq!(
                    kanban_task_label_style(&document, "kanban-palette-task"),
                    "color:#123456;fill:#123456;text-align:left !important"
                );
                let unknown = document
                    .descendants()
                    .find(|node| node.is_text() && node.text() == Some("Unknown"))
                    .expect("the unverified metadata is emitted");
                assert!(unknown.parent().is_some_and(|span| {
                    span.attribute("class") == Some("host")
                        || span.attribute("style") == Some("font-size:var(--host-size)")
                }));
                assert_eq!(
                    unknown.ancestors().any(|node| node.has_tag_name("a")),
                    field == "ticket",
                    "the ticket link retains the same unverified terminal"
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.accounted_count(), 1);
                assert_eq!(evidence.applied_count(), 0, "{field}: {metadata}");
                assert_eq!(evidence.not_applicable_count(), 0);
                assert_eq!(evidence.theme_residual_count(), 1, "{field}: {metadata}");

                let error = match try_render_kanban_with_theme_and_engine(
                    &source,
                    &theme,
                    engine(),
                    ThemePortabilityRequirement::RequirePortable,
                ) {
                    Ok(_) => panic!("a verified title cannot certify unknown {field} metadata"),
                    Err(error) => error,
                };
                assert_eq!(
                    error.unverified_family_theme(),
                    Some((merman_render::DiagramFamilyId::KANBAN, 1)),
                    "base={base_size}, {field}: {metadata}"
                );
            }
        }
    }
}

#[test]
fn kanban_task_label_unknown_metadata_only_invalidates_its_terminal_rule() {
    let source = concat!(
        "kanban\n  todo[Todo]\n",
        "    first[First]\n",
        "    second[Second]@{ assigned: \"Visible <span class='host'>Unknown</span>\" }\n",
    );
    for style_second in [false, true] {
        let mut rules = vec![
            ThemeRule::new(
                ThemeTarget::Task,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#fedcba").unwrap()),
            ),
            kanban_task_label_rule("#123456", 1),
        ];
        if style_second {
            rules.push(kanban_task_label_rule("#abcdef", 2));
        }
        let theme = kanban_task_rules_theme(rules);
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "securityLevel": "loose"
                    }),
                ),
            ),
            if style_second {
                ThemePortabilityRequirement::BestEffort
            } else {
                ThemePortabilityRequirement::RequirePortable
            },
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for id in ["kanban-palette-first", "kanban-palette-second"] {
            assert_eq!(kanban_task_rect_style(&document, id), "fill:#fedcba;");
        }
        assert_eq!(
            kanban_task_label_style(&document, "kanban-palette-first"),
            "color:#123456;fill:#123456;text-align:left !important"
        );
        assert_eq!(
            kanban_task_label_style(&document, "kanban-palette-second"),
            if style_second {
                "color:#abcdef;fill:#abcdef;text-align:left !important"
            } else {
                "text-align:left !important"
            }
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2 + usize::from(style_second));
        assert_eq!(evidence.accounted_count(), evidence.required_count());
        assert_eq!(evidence.applied_count(), 2);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), usize::from(style_second));
    }
}

#[test]
fn kanban_task_label_known_metadata_does_not_claim_or_block_source_paint() {
    let theme = kanban_task_label_theme("#123456");
    for field in ["ticket", "assigned"] {
        for metadata in [
            "<span style='color:#abcdef'>Owned</span>",
            "<span style='font-size:0px'>Hidden</span>",
        ] {
            for inherited_title in [false, true] {
                let title = if inherited_title {
                    "Task"
                } else {
                    "<span style='color:#abcdef'>Task</span>"
                };
                let source = format!(
                    "kanban\n  todo[Todo]\n    task[\"{title}\"]@{{ {field}: \"{metadata}\" }}\n"
                );
                let rendered = render_kanban_with_theme_and_engine(
                    &source,
                    &theme,
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({
                                "securityLevel": "loose",
                                "themeVariables": { "fontSize": "0.5px" }
                            }),
                        ),
                    ),
                    ThemePortabilityRequirement::RequirePortable,
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert_eq!(
                    kanban_task_label_style(&document, "kanban-palette-task"),
                    if inherited_title {
                        "color:#123456;fill:#123456;text-align:left !important"
                    } else {
                        "text-align:left !important"
                    }
                );
                let metadata_text = document
                    .descendants()
                    .find(|node| node.is_text() && matches!(node.text(), Some("Owned" | "Hidden")))
                    .expect("known source metadata remains serialized");
                assert!(metadata_text.parent().is_some_and(|span| {
                    matches!(
                        span.attribute("style"),
                        Some("color:#abcdef" | "font-size:0px")
                    )
                }));
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.accounted_count(), 1);
                assert_eq!(evidence.applied_count(), usize::from(inherited_title));
                assert_eq!(
                    evidence.not_applicable_count(),
                    usize::from(!inherited_title)
                );
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn kanban_typed_icon_task_label_preserves_the_label_background_class() {
    let theme = kanban_task_label_theme("#123456");
    let rendered = render_kanban_with_theme_and_engine(
        concat!(
            "kanban\n  todo[Todo]\n",
            "    task@{ icon: star, label: 'Task' }\n",
        ),
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid icon task Kanban SVG");

    assert_eq!(
        kanban_task_title_div_class(&document, "kanban-palette-task"),
        Some("labelBkg"),
        "typed foreground ownership must not change the icon task fallback DOM"
    );
    assert_eq!(
        kanban_task_label_style(&document, "kanban-palette-task"),
        "color:#123456;fill:#123456;text-align:left !important"
    );
}

#[test]
fn kanban_task_palette_is_not_applicable_without_tasks() {
    let theme = kanban_task_palette_theme(&["#123456"]);
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_task_palette_without_label_foregrounds_is_independently_portable() {
    let source = "kanban\n  todo[Todo]\n    task[Task]\n";
    let theme = kanban_task_palette_theme(&["#123456"]);

    let rendered = render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Kanban SVG");
    assert_eq!(
        kanban_task_rect_style(&document, "kanban-palette-task"),
        "fill:hsl(210, 65.3846153846%, 30.3921568627%);"
    );
    assert_eq!(
        kanban_task_label_style(&document, "kanban-palette-task"),
        "text-align:left !important"
    );
}

#[test]
fn kanban_task_palette_and_partial_label_rules_are_independent() {
    let source = concat!(
        "kanban\n  todo[Todo]\n",
        "    first[First]\n",
        "    second[Second]\n",
    );
    let styles =
        kanban_task_palette_styles(&["#123456"]).with_rule(kanban_task_label_rule("#000000", 1));
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile independently routed Kanban task styles");

    let rendered = render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid fallback Kanban surface SVG");
    for id in ["kanban-palette-first", "kanban-palette-second"] {
        assert_eq!(
            kanban_task_rect_style(&document, id),
            "fill:hsl(210, 65.3846153846%, 30.3921568627%);",
            "the task palette must apply independently to every visible card"
        );
    }
    assert_eq!(
        kanban_task_label_style(&document, "kanban-palette-first"),
        "color:#000000;fill:#000000;text-align:left !important"
    );
    assert_eq!(
        kanban_task_label_style(&document, "kanban-palette-second"),
        "text-align:left !important"
    );
}

#[test]
fn kanban_task_radius_clear_restores_the_mermaid_baseline() {
    let source = "kanban\n  todo[Todo]\n    task[Task]\n";
    let (_, svg) =
        render_kanban_svg_with_theme(source, &kanban_task_radius_theme(Specified::Clear));
    let document = roxmltree::Document::parse(&svg).expect("valid cleared Kanban SVG XML");
    let item = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some("kanban-markdown-task"))
        .and_then(|group| group.children().find(|node| node.has_tag_name("rect")))
        .expect("cleared Kanban task rect");
    assert_eq!(item.attribute("rx"), Some("5"));
    assert_eq!(item.attribute("ry"), Some("5"));
}

#[test]
fn kanban_task_radius_handles_duplicate_section_ids_without_replaying_items() {
    let source = concat!(
        "kanban\n",
        "  lane[First lane]\n",
        "    first[First task]\n",
        "  lane[Second lane]\n",
        "    second[Second task]\n",
    );
    let (layout, svg) =
        render_kanban_svg_with_theme(source, &kanban_task_radius_theme(Specified::Value(8.0)));

    assert_eq!(layout.sections.len(), 2);
    assert_eq!(layout.items.len(), 4);
    assert!(
        layout
            .items
            .iter()
            .all(|item| item.rx == 8.0 && item.ry == 8.0)
    );

    let document = roxmltree::Document::parse(&svg).expect("valid duplicate-section Kanban SVG");
    for id in ["kanban-markdown-first", "kanban-markdown-second"] {
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("g") && node.attribute("id") == Some(id))
                .count(),
            2,
            "each upstream model occurrence should be emitted exactly once"
        );
    }
}

#[test]
fn kanban_task_radius_rejects_qualified_rules_but_accepts_mixed_direct_paint() {
    let source = "kanban\n  todo[Todo]\n    task[Task]\n";
    let radius_style = kanban_task_radius_style(Specified::Value(8.0));
    for rule in [
        ThemeRule::new(ThemeTarget::Task, radius_style.clone()).with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::Task, radius_style.clone())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid task ordinal")),
        ThemeRule::new(
            ThemeTarget::Task,
            radius_style.clone().with_padding(InsetsPx::all(4.0)),
        ),
    ] {
        let error = try_render_kanban_svg_with_theme(source, &kanban_task_rules_theme([rule]))
            .expect_err("unsupported Kanban task routes must fail closed");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::KANBAN, 1))
        );
    }

    let mixed_direct = ThemeRule::new(
        ThemeTarget::Task,
        radius_style.with_stroke(CanvasPaint::solid("#123456").expect("valid task stroke")),
    );
    let (layout, svg) =
        try_render_kanban_svg_with_theme(source, &kanban_task_rules_theme([mixed_direct]))
            .expect("mixed direct Kanban radius and stroke should be portable");
    assert_eq!(layout.items[0].rx, 8.0);
    let document = roxmltree::Document::parse(&svg).expect("valid mixed direct Kanban SVG");
    assert_eq!(
        kanban_task_rect_style(&document, "kanban-markdown-task"),
        "stroke:#123456;"
    );
}

#[test]
fn kanban_text_fill_replaces_the_legacy_default_without_changing_other_surfaces() {
    let source =
        "kanban\n  todo[Todo]\n    task[Task]@{ ticket: ABC-7, assigned: Core, priority: High }\n";
    for look in ["classic", "neo", "handDrawn"] {
        for variant in [None, Some(ThemeVariant::Default)] {
            for (paint, color) in [
                (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                (CanvasPaint::Transparent, "transparent"),
            ] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(paint),
                );
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let config = serde_json::json!({
                    "look": look,
                    "kanban": { "ticketBaseUrl": "https://example.test/#TICKET#" }
                });
                let rendered = render_kanban_with_theme_and_engine(
                    source,
                    &kanban_task_rules_theme([rule]),
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            config.clone(),
                        ),
                    ),
                    ThemePortabilityRequirement::RequirePortable,
                );
                let mut legacy_config = config;
                legacy_config["themeVariables"] = serde_json::json!({
                    "textColor": color,
                    "taskTextColor": color,
                });
                let legacy = render_kanban_with_theme_and_engine(
                    source,
                    &kanban_task_rules_theme([]),
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            legacy_config,
                        ),
                    ),
                    ThemePortabilityRequirement::RequirePortable,
                );
                assert_eq!(
                    rendered.svg(),
                    legacy.svg(),
                    "look={look}, variant={variant:?}, color={color}"
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let css = document
                    .root_element()
                    .children()
                    .find(|node| node.has_tag_name("style"))
                    .unwrap()
                    .text()
                    .unwrap();
                assert!(css.contains(&format!(
                    "#kanban-palette .cluster-label,#kanban-palette .label{{color:{color};fill:{color};}}"
                )));
                let root_rule = css
                    .split('}')
                    .find(|rule| rule.starts_with("#kanban-palette{"))
                    .unwrap();
                assert!(root_rule.contains(&format!("fill:{color};")));
                for label in ["Todo", "Task", "ABC-7", "Core"] {
                    assert!(
                        document
                            .descendants()
                            .any(|node| node.is_text() && node.text() == Some(label)),
                        "{label}"
                    );
                }
                assert!(document.descendants().any(|node| {
                    node.has_tag_name("a")
                        && node
                            .attributes()
                            .any(|attr| attr.name() == "href" && attr.value().contains("ABC-7"))
                }));
                let completion = rendered.into_completion();
                let evidence = merman_render::__private::family_evidence(completion.report());
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn kanban_text_fill_preserves_config_and_more_specific_label_owners() {
    let source = "kanban\n  todo[Todo]\n    task[Task]\n";
    let text_rule = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    );
    let theme = kanban_task_rules_theme([text_rule, kanban_task_label_rule("#abcdef", 1)]);
    for config_owned in [false, true] {
        let config = if config_owned {
            serde_json::json!({"themeVariables": {"textColor": "#fedcba"}})
        } else {
            serde_json::json!({})
        };
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            kanban_task_label_style(&document, "kanban-palette-task"),
            if config_owned {
                "text-align:left !important"
            } else {
                "color:#abcdef;fill:#abcdef;text-align:left !important"
            }
        );
        let css = document
            .root_element()
            .children()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        let color = if config_owned { "#fedcba" } else { "#123456" };
        assert!(css.contains(&format!(
            "#kanban-palette .cluster-label,#kanban-palette .label{{color:{color};fill:{color};}}"
        )));
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 2);
        assert_eq!(evidence.applied_count(), if config_owned { 0 } else { 2 });
        assert_eq!(
            evidence.not_applicable_count(),
            if config_owned { 2 } else { 0 }
        );
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn kanban_text_fill_has_no_application_without_inheriting_label_terminals() {
    let theme = kanban_task_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    for source in [
        "kanban\n",
        "---\ntitle: Metadata only\n---\nkanban\n",
        "%%{init: {\"securityLevel\": \"loose\"}}%%\nkanban\n  todo[\"<span style='color:#abcdef'>Owned</span>\"]\n",
    ] {
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{source}");
        assert_eq!(evidence.applied_count(), 0, "{source}");
        assert_eq!(evidence.not_applicable_count(), 1, "{source}");
        assert_eq!(evidence.theme_residual_count(), 0, "{source}");
    }
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let completion = rendered.into_completion();
    assert_eq!(
        merman_render::__private::family_evidence(completion.report()).applied_count(),
        1
    );
}

#[test]
fn kanban_text_fill_does_not_certify_unsupported_siblings_or_unknown_source_color() {
    let fill = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    let mixed = ThemeStylePatch {
        typography: merman_render::diagram_theme::TextStylePatch {
            font_stack: Specified::Value(FontStack::single("UnsupportedRuleFont").unwrap()),
            ..Default::default()
        },
        ..fill.clone()
    };
    for (source, patch) in [
        ("kanban\n  todo[Todo]\n    task[Task]\n", mixed),
        (
            "%%{init: {\"securityLevel\": \"loose\"}}%%\nkanban\n  todo[\"Visible <span class='label'>Unknown</span>\"]\n",
            fill,
        ),
    ] {
        let theme = kanban_task_rules_theme([ThemeRule::new(ThemeTarget::Text, patch)]);
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        let result = try_render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        assert!(matches!(
            result,
            Err(merman_render::Error::UnverifiedFamilyTheme { .. })
        ));
    }
}

#[test]
fn kanban_text_effect_binding_requires_visible_text_before_reporting_a_residual() {
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
    };
    let effects = DiagramEffectSet::default()
        .with_graph(
            EffectGraph::new(
                "blur",
                [EffectPrimitive::GaussianBlur {
                    input: EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                }],
            )
            .unwrap(),
        )
        .unwrap()
        .with_binding(EffectBinding::new(ThemeTarget::Text, "blur").unwrap())
        .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_effects(effects))
        .unwrap();
    for (source, visible) in [("kanban\n", false), ("kanban\n  todo[Todo]\n", true)] {
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.not_applicable_count(), usize::from(!visible));
        assert_eq!(evidence.theme_residual_count(), usize::from(visible));
        let strict = try_render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        if visible {
            assert!(matches!(
                strict,
                Err(merman_render::Error::UnverifiedFamilyTheme { .. })
            ));
        } else {
            strict.expect("empty Text domain is not applicable");
        }
    }
}

#[test]
fn kanban_title_fill_is_not_applicable_without_a_diagram_title_terminal() {
    let baseline_theme = kanban_task_rules_theme([]);
    for source in [
        "kanban\n",
        "kanban\n  todo[Todo]\n    task[Task]\n",
        "---\ntitle: Metadata title\n---\nkanban\n  todo[Todo]\n    task[Task]\n",
        "kanban\n  title Release\n    task[Task]\n",
    ] {
        let baseline = render_kanban_with_theme_and_engine(
            source,
            &baseline_theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        for variant in [None, Some(ThemeVariant::Default)] {
            for paint in [
                CanvasPaint::solid("#987654").expect("valid title fill"),
                CanvasPaint::Transparent,
            ] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(paint),
                );
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let rendered = render_kanban_with_theme_and_engine(
                    source,
                    &kanban_task_rules_theme([rule]),
                    MermaidEngine::new(),
                    ThemePortabilityRequirement::RequirePortable,
                );
                assert_eq!(rendered.svg(), baseline.svg(), "source={source:?}");
                let document =
                    roxmltree::Document::parse(rendered.svg()).expect("valid Kanban SVG");
                assert!(
                    !document
                        .descendants()
                        .any(|node| { node.is_text() && node.text() == Some("Metadata title") })
                );
                if source.contains("  title Release\n") {
                    let column_label = document
                        .descendants()
                        .find(|node| node.attribute("class") == Some("cluster-label"))
                        .expect("literal title statement creates a column label");
                    assert!(
                        column_label
                            .descendants()
                            .any(|node| { node.is_text() && node.text() == Some("title Release") })
                    );
                }
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
}

#[test]
fn kanban_all_presets_keep_metadata_titles_nonvisual_and_fully_accounted() {
    let presets = merman_render::diagram_theme::theme_preset_descriptors();
    assert_eq!(presets.len(), 11);
    let source =
        "---\ntitle: Nonvisual metadata title\n---\nkanban\n  todo[Todo]\n    task[Task]\n";
    for preset in presets {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset.preset())
            .unwrap();
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for label in ["Todo", "Task"] {
            assert!(
                document
                    .descendants()
                    .any(|node| node.is_text() && node.text() == Some(label))
            );
        }
        assert!(
            !document
                .descendants()
                .any(|node| { node.is_text() && node.text() == Some("Nonvisual metadata title") })
        );
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

#[test]
fn kanban_legacy_title_color_has_no_consumer_while_text_color_styles_labels() {
    let source = concat!(
        "---\ntitle: Metadata title\n---\n",
        "kanban\n  title Release\n    task[Task]\n",
    );
    let render = |title_color: &str, text_color: &str| {
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": {
                            "titleColor": title_color,
                            "textColor": text_color
                        }
                    }),
                ),
            )
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse ordinary configured Kanban")
            .expect("detect Kanban")
            .into_presentation();
        assert_eq!(
            parsed
                .metadata()
                .effective_config
                .get_str("themeVariables.titleColor"),
            Some(title_color),
            "the legacy config probe must reach the effective configuration"
        );
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin unthemed Kanban session");
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
            .expect("prepare ordinary configured Kanban");
        render_kanban_artifact(artifact)
            .expect("render ordinary configured Kanban")
            .1
    };
    let first = render("#123456", "#abcdef");
    let second = render("#987654", "#abcdef");
    assert_eq!(first, second, "titleColor must not change any emitted SVG");
    let changed_text = render("#123456", "#fedcba");
    assert_ne!(first, changed_text, "textColor has real label consumers");
    for (svg, text_color) in [(&first, "#abcdef"), (&changed_text, "#fedcba")] {
        let document = roxmltree::Document::parse(svg).expect("valid configured Kanban SVG");
        let stylesheet = document
            .root_element()
            .children()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("root Kanban stylesheet");
        assert!(stylesheet.contains(&format!(
            "#kanban-markdown .cluster-label,#kanban-markdown .label{{color:{text_color};fill:{text_color};}}"
        )));
        let column_label = document
            .descendants()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .expect("visible column label");
        assert!(
            column_label
                .descendants()
                .any(|node| { node.is_text() && node.text() == Some("title Release") })
        );
        let card = document
            .descendants()
            .find(|node| node.attribute("id") == Some("kanban-markdown-task"))
            .expect("visible task card");
        assert!(
            card.descendants()
                .any(|node| { node.is_text() && node.text() == Some("Task") })
        );
        assert!(
            !document
                .descendants()
                .any(|node| { node.is_text() && node.text() == Some("Metadata title") })
        );
    }
}

#[test]
fn kanban_task_radius_is_not_applicable_without_items() {
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n",
        &kanban_task_radius_theme(Specified::Value(8.0)),
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_configured_diagram_and_root_fonts_preserve_typography_binding_semantics() {
    for (root_font, typed_size, expected_size, expected_root) in [
        ("Root, serif", false, "16px", Some("Root,serif")),
        ("", false, "16px", None),
        ("Root, serif", true, "23px", Some("Diagram,monospace")),
        ("", true, "23px", Some("Diagram,monospace")),
    ] {
        let style = if typed_size {
            ThemeTextStyle::default()
                .with_font_size_px(23.0)
                .expect("valid Kanban font size")
        } else {
            ThemeTextStyle::default()
        };
        let theme = kanban_typography_theme(style);
        let (_, rendered) = try_render_kanban_with_layout_and_environment(
            "kanban\n  todo[Todo]\n    task[Task]",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "fontFamily": root_font,
                        "themeVariables": { "fontFamily": "Diagram, monospace" }
                    }),
                ),
            ),
            ThemePortabilityRequirement::BestEffort,
            RenderEnvironment::deterministic(),
        )
        .expect("render configured Kanban fonts");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Kanban SVG");
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Kanban stylesheet");
        assert!(
            css.contains(&format!(
                "#kanban-palette{{font-family:Diagram,monospace;font-size:{expected_size};"
            )),
            "root_font={root_font:?}, typed_size={typed_size}: {css}"
        );
        match expected_root {
            Some(family) => assert!(
                css.contains(&format!(
                    "#kanban-palette :root{{--mermaid-font-family:{family};}}"
                )),
                "root_font={root_font:?}, typed_size={typed_size}: {css}"
            ),
            None => assert!(
                !css.contains(":root{--mermaid-font-family:"),
                "root_font={root_font:?}, typed_size={typed_size}: {css}"
            ),
        }
    }
}

#[test]
fn kanban_root_font_size_preserves_numeric_and_css_unit_fallbacks() {
    for (font_size, expected_css) in [
        (serde_json::json!(0), "0px"),
        (serde_json::json!(0.5), "0.5px"),
        (serde_json::json!(20), "20px"),
        (serde_json::json!("0px"), "16px"),
        (serde_json::json!("0.5px"), "16px"),
        (serde_json::json!("20px"), "16px"),
    ] {
        let source = format!(
            "---\nconfig: {}\n---\nkanban\n  todo[Todo]\n    task[Task]\n",
            serde_json::json!({
                "fontSize": font_size,
                "themeVariables": { "fontSize": "" }
            })
        );
        let (layout, svg) = parse_layout_and_render(&source);
        let expected_size: f64 = expected_css.strip_suffix("px").unwrap().parse().unwrap();
        assert_eq!(layout.items[0].height, 20.0 + 1.5 * expected_size);
        let document = roxmltree::Document::parse(&svg).expect("valid Kanban SVG");
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Kanban stylesheet");
        let root_rule = css
            .split('}')
            .find(|rule| rule.starts_with("#kanban-markdown{"))
            .expect("Kanban root CSS rule");
        assert!(
            root_rule.contains(&format!("font-size:{expected_css};")),
            "font_size={font_size}: {root_rule}"
        );
    }
}

#[test]
fn kanban_source_and_site_nonnegative_sizes_share_geometry_css_and_receipts() {
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("KanbanTyped").expect("valid font stack"))
            .with_font_size_px(24.0)
            .expect("valid Kanban font size"),
    );
    let board = "kanban\n  todo[Todo]\n    task[Task]\n";
    for from_source in [false, true] {
        for html_labels in [false, true] {
            for (value, size) in [
                (serde_json::json!(0), 0.0),
                (serde_json::json!("0px"), 0.0),
                (serde_json::json!(0.5), 0.5),
                (serde_json::json!("0.5px"), 0.5),
            ] {
                let config = serde_json::json!({
                    "htmlLabels": html_labels,
                    "themeVariables": { "fontSize": value }
                });
                let (source, engine) = if from_source {
                    (
                        format!("---\nconfig: {config}\n---\n{board}"),
                        MermaidEngine::new(),
                    )
                } else {
                    (
                        board.to_owned(),
                        MermaidEngine::new().with_site_config(
                            merman_core::presentation::SourcePresentationConfig::from_value(config),
                        ),
                    )
                };
                let (layout, rendered) = try_render_kanban_with_layout_and_environment(
                    &source,
                    &theme,
                    engine,
                    ThemePortabilityRequirement::RequirePortable,
                    RenderEnvironment::deterministic(),
                )
                .expect("render inherited zero/subpixel Kanban size");
                let context =
                    format!("source={from_source}, htmlLabels={html_labels}, fontSize={value}");
                assert_eq!(
                    kanban_root_font_size_css(rendered.svg(), "kanban-palette"),
                    format!("{size}px"),
                    "{context}"
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let expected_height = 20.0 + 1.5 * size;
                assert_eq!(layout.items[0].height, expected_height, "{context}");
                assert_eq!(
                    layout.sections[0].rect_height,
                    55.0 + 1.5 * size,
                    "{context}"
                );
                assert_eq!(
                    kanban_task_rect_height(&document, "kanban-palette-task"),
                    expected_height,
                    "{context}"
                );
                let (width, height) =
                    kanban_task_label_foreign_object(&document, "kanban-palette-task", "Task");
                assert_eq!(height, 24.0, "fixed Kanban label wrapper: {context}");
                if size == 0.0 {
                    assert_eq!(width, 0.0, "zero-size glyphs reserve no width: {context}");
                } else {
                    assert!(width > 0.0 && width < 10.0, "{context}: width={width}");
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 2, "{context}");
                assert_eq!(evidence.accounted_count(), 2, "{context}");
                assert_eq!(
                    evidence.applied_count(),
                    usize::from(size > 0.0),
                    "{context}"
                );
                assert_eq!(
                    evidence.not_applicable_count(),
                    if size == 0.0 { 2 } else { 1 },
                    "zero text must not certify the typed font stack: {context}"
                );
                assert_eq!(evidence.theme_residual_count(), 0, "{context}");
            }
        }
    }
}

#[test]
fn kanban_invalid_theme_size_falls_back_to_valid_root_or_default_without_a_theme() {
    for from_source in [false, true] {
        for (theme_size, root_size, expected_size) in [
            (serde_json::json!(-2), serde_json::json!(0), 0.0),
            (serde_json::json!("-2px"), serde_json::json!(0.5), 0.5),
            (serde_json::json!("bogus"), serde_json::json!(0.5), 0.5),
            (serde_json::json!(""), serde_json::json!(0.5), 0.5),
            (serde_json::json!(-2), serde_json::json!(-3), 16.0),
        ] {
            let config = serde_json::json!({
                "fontSize": root_size,
                "themeVariables": { "fontSize": theme_size }
            });
            let board = "kanban\n  todo[Todo]\n    task[Task]\n";
            let (source, engine) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{board}"),
                    MermaidEngine::new(),
                )
            } else {
                (
                    board.to_owned(),
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                )
            };
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let artifact = family::prepare(
                &parsed,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .expect("prepare unthemed Kanban size fallback");
            let (layout, svg) = render_kanban_artifact(artifact).unwrap();
            assert_eq!(
                kanban_root_font_size_css(&svg, "kanban-markdown"),
                format!("{expected_size}px"),
                "source={from_source}, theme={theme_size}, root={root_size}"
            );
            assert_eq!(layout.items[0].height, 20.0 + 1.5 * expected_size);
        }
    }
}

#[test]
fn kanban_unrequested_typography_measures_the_canonical_common_css_size() {
    let theme = kanban_task_rules_theme([ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
    )]);
    for from_source in [false, true] {
        let config = serde_json::json!({
            "themeVariables": { "fontSize": "0.00000001px" }
        });
        let board = "kanban\n  todo[Todo]\n    task[Task]\n";
        let (source, engine) = if from_source {
            (
                format!("---\nconfig: {config}\n---\n{board}"),
                MermaidEngine::new(),
            )
        } else {
            (
                board.to_owned(),
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                ),
            )
        };
        let (layout, rendered) = try_render_kanban_with_layout_and_environment(
            &source,
            &theme,
            engine,
            ThemePortabilityRequirement::RequirePortable,
            RenderEnvironment::deterministic(),
        )
        .expect("bind unrequested Kanban typography to its actual common CSS size");
        assert_eq!(
            kanban_root_font_size_css(rendered.svg(), "kanban-palette"),
            "0px"
        );
        assert_eq!(layout.items[0].height, 20.0);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            kanban_task_label_foreign_object(&document, "kanban-palette-task", "Task").0,
            0.0
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
fn kanban_typed_font_size_reaches_measurement_layout_css_and_evidence() {
    let source = concat!(
        "kanban\n",
        "  todo[Todo]\n",
        "    task[Task]@{ ticket: MC-2038, assigned: 'Alice' }\n",
        "    next[Next]\n",
    );
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Kanban font size"),
    );
    let (environment, expected_size_calls, unexpected_size_calls) =
        kanban_probe_environment(24.0, "test.kanban-typed-font-size");
    let (layout, rendered) = try_render_kanban_with_layout_and_environment(
        source,
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "fontSize": 30
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
        environment,
    )
    .expect("render typed Kanban font size");

    assert!(expected_size_calls.load(Ordering::Relaxed) > 0);
    assert_eq!(unexpected_size_calls.load(Ordering::Relaxed), 0);
    assert!(
        rendered.svg().contains("font-size:24px"),
        "typed Kanban size must reach the shared stylesheet: {}",
        rendered.svg()
    );
    assert!(
        !rendered.svg().contains("font-size:30px"),
        "root fontSize is a fallback and must not override typed Kanban FontSize"
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid typed Kanban SVG");
    assert_eq!(
        kanban_task_rect_height(&document, "kanban-palette-task"),
        74.0,
        "typed Kanban size must use title/detail bbox geometry and fixed Mermaid padding"
    );
    assert_eq!(layout.items.len(), 2);
    assert_eq!(layout.items[0].height, 74.0);
    assert_eq!(layout.items[1].height, 48.0);
    assert_eq!(layout.items[0].center_y, -231.0);
    assert_eq!(layout.items[1].center_y, -165.0);
    assert_eq!(layout.max_label_height, 32.0);
    assert_eq!(layout.sections[0].rect_height, 169.0);
    assert_eq!(
        kanban_task_label_foreign_object(&document, "kanban-palette-task", "MC-2038"),
        (60.0, 24.0),
        "ticket geometry must be prepared once and retain Mermaid's minimum label height"
    );
    assert_eq!(
        kanban_task_label_foreign_object(&document, "kanban-palette-task", "Alice"),
        (50.0, 34.0),
        "assigned geometry must be emitted from the prepared detail bbox"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_typed_subpixel_size_reaches_the_measurement_callback_css_and_receipt() {
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(0.5)
            .expect("positive typed subpixel font size"),
    );
    let (environment, expected_size_calls, unexpected_size_calls) =
        kanban_probe_environment(0.5, "test.kanban-typed-subpixel-font-size");
    let (layout, rendered) = try_render_kanban_with_layout_and_environment(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "fontSize": 30,
                "kanban": { "fontSize": 0 }
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
        environment,
    )
    .expect("render typed subpixel Kanban font size");
    assert!(expected_size_calls.load(Ordering::Relaxed) > 0);
    assert_eq!(unexpected_size_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        kanban_root_font_size_css(rendered.svg(), "kanban-palette"),
        "0.5px"
    );
    assert_eq!(
        layout.items[0].height, 33.5,
        "probe title height plus fixed padding"
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert_eq!(
        kanban_task_rect_height(&document, "kanban-palette-task"),
        33.5
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_typed_font_size_yields_to_explicit_theme_variable_ownership() {
    let source = concat!(
        "kanban\n",
        "  todo[Todo]\n",
        "    task[Task]@{ ticket: MC-2038, assigned: 'Alice' }\n",
        "    next[Next]\n",
    );
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Kanban font size"),
    );
    let (environment, expected_size_calls, unexpected_size_calls) =
        kanban_probe_environment(30.0, "test.kanban-config-font-size");
    let (layout, rendered) = try_render_kanban_with_layout_and_environment(
        source,
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "fontSize": "30px" }
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
        environment,
    )
    .expect("render Kanban with config-owned font size");

    assert!(expected_size_calls.load(Ordering::Relaxed) > 0);
    assert_eq!(unexpected_size_calls.load(Ordering::Relaxed), 0);
    assert!(rendered.svg().contains("font-size:30px"));
    assert!(!rendered.svg().contains("font-size:24px"));
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid config-owned SVG");
    assert_eq!(layout.items[0].height, 83.0);
    assert_eq!(layout.items[1].height, 54.0);
    assert_eq!(layout.items[0].center_y, -220.5);
    assert_eq!(layout.items[1].center_y, -147.0);
    assert_eq!(layout.max_label_height, 38.0);
    assert_eq!(layout.sections[0].rect_height, 190.0);
    assert_eq!(
        kanban_task_rect_height(&document, "kanban-palette-task"),
        83.0,
        "config-owned font size must drive the same prepared layout as its CSS"
    );
    assert_eq!(
        kanban_task_label_foreign_object(&document, "kanban-palette-task", "Alice"),
        (50.0, 40.0)
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_invalid_explicit_font_size_does_not_displace_the_typed_owner() {
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(0.5)
            .expect("valid Kanban font size"),
    );
    for from_source in [false, true] {
        for value in [
            serde_json::json!(-2),
            serde_json::json!("-2px"),
            serde_json::json!("bogus"),
            serde_json::json!(""),
            serde_json::Value::Null,
        ] {
            let config = serde_json::json!({
                "fontSize": 30,
                "themeVariables": { "fontSize": value }
            });
            let board = "kanban\n  todo[Todo]\n    task[Task]\n";
            let (source, engine) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{board}"),
                    MermaidEngine::new(),
                )
            } else {
                (
                    board.to_owned(),
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                )
            };
            let (environment, expected_size_calls, unexpected_size_calls) =
                kanban_probe_environment(0.5, "test.kanban-invalid-config-font-size");
            let (_, rendered) = try_render_kanban_with_layout_and_environment(
                &source,
                &theme,
                engine,
                ThemePortabilityRequirement::RequirePortable,
                environment,
            )
            .expect("invalid font-size candidate must leave the typed owner active");
            assert!(expected_size_calls.load(Ordering::Relaxed) > 0);
            assert_eq!(unexpected_size_calls.load(Ordering::Relaxed), 0);
            assert_eq!(
                kanban_root_font_size_css(rendered.svg(), "kanban-palette"),
                "0.5px"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(
                evidence.required_count(),
                1,
                "source={from_source}, value={value}"
            );
            assert_eq!(
                evidence.applied_count(),
                1,
                "source={from_source}, value={value}"
            );
            assert_eq!(evidence.not_applicable_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn kanban_unmeasured_explicit_font_size_retains_its_portability_residual() {
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Kanban font size"),
    );

    for (label, value) in [
        ("relative unit", serde_json::json!("1em")),
        ("host expression", serde_json::json!("var(--kanban-size)")),
    ] {
        let source = "kanban\n  todo[Todo]\n    task[Task]\n";
        let rendered = render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "fontSize": value }
                    }),
                ),
            ),
            ThemePortabilityRequirement::BestEffort,
        );
        assert!(
            rendered.svg().contains("font-size:16px"),
            "unmeasured configured size retains its existing fallback in BestEffort mode: {label}"
        );
        assert!(!rendered.svg().contains("font-size:24px"));

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "value={label}");
        assert_eq!(evidence.accounted_count(), 1, "value={label}");
        assert_eq!(evidence.applied_count(), 0, "value={label}");
        assert_eq!(evidence.not_applicable_count(), 0, "value={label}");
        assert_eq!(evidence.theme_residual_count(), 1, "value={label}");

        let error = match try_render_kanban_with_theme_and_engine(
            source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "fontSize": value }
                    }),
                ),
            ),
            ThemePortabilityRequirement::RequirePortable,
        ) {
            Ok(_) => panic!("unmeasured source size must fail closed in portable mode"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::KANBAN, 1)),
            "value={label}"
        );
    }
}

#[test]
fn kanban_unsupported_typography_is_property_local_to_the_typed_base_properties() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("KanbanTyped").expect("valid Kanban font stack"))
        .with_font_size_px(24.0)
        .expect("valid Kanban font size")
        .with_font_weight(700)
        .expect("valid unsupported Kanban font weight");
    let theme = kanban_typography_theme(typography);
    let source = "kanban\n  todo[Todo]\n    task[Task]\n";
    let rendered = render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );

    assert!(rendered.svg().contains("font-family:KanbanTyped"));
    assert!(rendered.svg().contains("font-size:24px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 1);

    let error = match try_render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    ) {
        Ok(_) => panic!("strict Kanban must reject unsupported font weight"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::KANBAN, 1))
    );
}

#[test]
fn kanban_source_owned_font_size_is_a_property_local_portability_residual() {
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Kanban font size"),
    );
    let source = concat!(
        "%%{init: {\"securityLevel\": \"loose\"}}%%\n",
        "kanban\n  todo[Todo]\n",
        "    task[\"Visible <span style='font-size:30px'>Source sized</span>\"]\n",
    );
    let rendered = render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    assert!(rendered.svg().contains("font-size:24px"));
    assert!(rendered.svg().contains("font-size:30px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let error = match try_render_kanban_with_theme_and_engine(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    ) {
        Ok(_) => panic!("source-owned Kanban font size must fail closed in portable mode"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::KANBAN, 1))
    );
}

#[test]
fn kanban_zero_base_preserves_local_absolute_sizes_and_independent_font_ownership() {
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("KanbanTyped").expect("valid font stack"))
            .with_font_size_px(24.0)
            .expect("valid Kanban font size"),
    );
    for (label, expected_height, expected_applied, expected_residual) in [
        ("Hidden", 20.0, 0, 0),
        ("<span style='font-size:0px'>Hidden</span>", 20.0, 0, 0),
        ("<span style='font-size:0rem'>Hidden</span>", 20.0, 0, 0),
        ("<span style='font-size:200%'>Hidden</span>", 20.0, 0, 0),
        ("<big><small>Hidden</small></big>", 20.0, 0, 0),
        ("<span style='font-size:-2px'>Hidden</span>", 20.0, 0, 0),
        (
            "<span style='font-size:0 !important;font-size:12px'>Hidden</span>",
            20.0,
            0,
            0,
        ),
        (
            "Hidden <span style='font-size:12px'>Restored</span>",
            38.0,
            1,
            0,
        ),
        (
            "<span style='font-size:12px'><span style='font-size:0'>Hidden</span>Restored</span>",
            38.0,
            1,
            0,
        ),
        (
            "<span style='font-size:12px;font-size:-2px !important'>Restored</span>",
            38.0,
            1,
            0,
        ),
        (
            "<small style='font-size:12px'><big style='font-size:inherit'>Restored</big></small>",
            38.0,
            1,
            0,
        ),
        (
            "<span style='font-size:12px;font-family:LocalFont'>Restored</span>",
            38.0,
            0,
            1,
        ),
        (
            "<span style='font-size:var(--local-size)'>Uncertain</span>",
            20.0,
            0,
            1,
        ),
        (
            "<span style='font-size:var(--local-size) !important;font-size:0px'>Uncertain</span>",
            20.0,
            0,
            1,
        ),
        ("<span style='font-size:1rem'>Restored</span>", 44.0, 0, 1),
        ("<span class='local-label'>Uncertain</span>", 20.0, 0, 1),
    ] {
        let source = format!("kanban\n  todo[Todo]\n    task[\"{label}\"]\n");
        let portability = if expected_residual == 0 {
            ThemePortabilityRequirement::RequirePortable
        } else {
            ThemePortabilityRequirement::BestEffort
        };
        let (layout, rendered) = try_render_kanban_with_layout_and_environment(
            &source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "securityLevel": "loose",
                        "themeVariables": { "fontSize": "0px" }
                    }),
                ),
            ),
            portability,
            RenderEnvironment::deterministic(),
        )
        .expect("render zero-base Kanban with independent local typography");
        assert_eq!(
            kanban_root_font_size_css(rendered.svg(), "kanban-palette"),
            "0px"
        );
        assert_eq!(layout.items[0].height, expected_height, "label={label}");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            kanban_task_rect_height(&document, "kanban-palette-task"),
            expected_height,
            "label={label}"
        );
        assert!(
            document.descendants().any(|node| {
                node.is_text()
                    && node.text().is_some_and(|text| {
                        text.contains("Hidden") || text.contains("Restored") || text == "Uncertain"
                    })
            }),
            "visibility receipts must not remove serialized source text: {label}"
        );
        if expected_height > 20.0 {
            let restored = document
                .descendants()
                .find(|node| node.is_text() && node.text() == Some("Restored"))
                .expect("locally restored text");
            let foreign_object = restored
                .ancestors()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            assert!(
                foreign_object
                    .attribute("width")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
                    > 0.0
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2, "label={label}");
        assert_eq!(evidence.accounted_count(), 2, "label={label}");
        assert_eq!(evidence.applied_count(), expected_applied, "label={label}");
        assert_eq!(
            evidence.theme_residual_count(),
            expected_residual,
            "label={label}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            2 - expected_applied - expected_residual,
            "label={label}"
        );
    }
}

#[test]
fn kanban_zero_inline_text_does_not_certify_positive_typed_fonts() {
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("KanbanTyped").expect("valid font stack"))
            .with_font_size_px(0.5)
            .expect("valid Kanban subpixel font size"),
    );
    let rendered = render_kanban_with_theme_and_engine(
        concat!(
            "kanban\n",
            "  todo[\"<span style='font-size:0px'>Todo</span>\"]\n",
            "    task[\"<span style='font-size:0px'>Task</span>\"]\n",
        ),
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "securityLevel": "loose"
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
    );
    assert_eq!(
        kanban_root_font_size_css(rendered.svg(), "kanban-palette"),
        "0.5px"
    );
    assert!(rendered.svg().contains("font-size:0px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_zero_text_does_not_certify_label_paint_or_suppress_independent_card_paint() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        merman_render::DiagramFamilyId::KANBAN,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("KanbanTyped").unwrap())
                            .with_font_size_px(0.5)
                            .unwrap(),
                    ),
                )
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Task,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::TaskLabel,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#abcdef").unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#654321").unwrap()),
                        )),
                ),
        )
        .unwrap();
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "fontSize": "0px" }
            })),
        ),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(kanban_task_rect_style(&document, "kanban-palette-task").contains("fill:#123456"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 5);
    assert_eq!(evidence.accounted_count(), 5);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 4);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn kanban_zero_base_allows_ticket_and_assigned_absolute_sizes_to_restore_visibility() {
    let theme = kanban_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("KanbanTyped").expect("valid font stack")),
    );
    for field in ["ticket", "assigned"] {
        let source = format!(
            "kanban\n  todo[Todo]\n    task[Task]@{{ {field}: \"<span style='font-size:12px'>Restored</span>\" }}\n"
        );
        let (layout, rendered) = try_render_kanban_with_layout_and_environment(
            &source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "securityLevel": "loose",
                        "themeVariables": { "fontSize": "0px" }
                    }),
                ),
            ),
            ThemePortabilityRequirement::RequirePortable,
            RenderEnvironment::deterministic(),
        )
        .expect("metadata can restore visible text below a zero Kanban base");
        assert_eq!(
            kanban_root_font_size_css(rendered.svg(), "kanban-palette"),
            "0px"
        );
        assert_eq!(
            layout.items[0].height, 29.0,
            "{field} detail contributes half its height"
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let restored = document
            .descendants()
            .find(|node| node.is_text() && node.text() == Some("Restored"))
            .expect("visible prepared metadata");
        let foreign_object = restored
            .ancestors()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert!(
            foreign_object
                .attribute("width")
                .unwrap()
                .parse::<f64>()
                .unwrap()
                > 0.0
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1, "{field}");
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn kanban_source_owned_facet_preserves_other_property_application() {
    for (patch, config, expected_applied, expected_style) in [
        (
            kanban_task_radius_style(Specified::Value(8.0))
                .with_fill(CanvasPaint::solid("#123456").unwrap()),
            serde_json::json!({"themeVariables":{"background":"#abcdef"}}),
            1,
            None,
        ),
        (
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").unwrap())
                .with_stroke(CanvasPaint::solid("#654321").unwrap()),
            serde_json::json!({"themeVariables":{"background":"#abcdef"}}),
            1,
            Some("stroke:#654321"),
        ),
        (
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").unwrap())
                .with_stroke(CanvasPaint::solid("#654321").unwrap()),
            serde_json::json!({"themeVariables":{"background":"#abcdef","nodeBorder":"#fedcba"}}),
            0,
            None,
        ),
    ] {
        let has_radius = matches!(patch.geometry.radius, Specified::Value(_));
        let theme = kanban_task_rules_theme([ThemeRule::new(ThemeTarget::Task, patch)]);
        let rendered = render_kanban_with_theme_and_engine(
            "kanban\n  todo[Todo]\n    task[Task]",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
            ThemePortabilityRequirement::RequirePortable,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let style = kanban_task_rect_style(&document, "kanban-palette-task");
        assert!(!style.contains("fill:#123456"));
        if let Some(expected) = expected_style {
            assert!(style.contains(expected), "{style}");
        }
        if has_radius {
            let rect = document
                .descendants()
                .find(|node| node.attribute("id") == Some("kanban-palette-task"))
                .unwrap()
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            assert_eq!(rect.attribute("rx"), Some("8"));
            assert_eq!(rect.attribute("ry"), Some("8"));
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), expected_applied);
        assert_eq!(evidence.not_applicable_count(), 1 - expected_applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn repair_u7_kanban_source_owned_fill_keeps_unsupported_radius_residual() {
    let theme = kanban_task_rules_theme([ThemeRule::new(
        ThemeTarget::Task,
        kanban_task_radius_style(Specified::Value(8.0))
            .with_fill(CanvasPaint::solid("#123456").unwrap()),
    )
    .with_ordinal(OrdinalSelector::Exact(1))]);
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(
            serde_json::json!({"themeVariables":{"background":"#abcdef"}}),
        ),
    );
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]",
        &theme,
        engine.clone(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert!(
        try_render_kanban_with_theme_and_engine(
            "kanban\n  todo[Todo]\n    task[Task]",
            &theme,
            engine,
            ThemePortabilityRequirement::RequirePortable
        )
        .is_err()
    );
}

#[test]
fn repair_u7_kanban_configured_font_precedence_matches_prepared_layout_and_css() {
    let theme = kanban_task_rules_theme([]);
    let (layout, rendered) = try_render_kanban_with_layout_and_environment("kanban\n  todo[Todo]\n    task[Task]", &theme,
        MermaidEngine::new().with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "fontFamily":"RootFallback, serif","themeVariables":{"fontFamily":"Configured, sans-serif","fontSize":"22px"}}))),
        ThemePortabilityRequirement::RequirePortable, RenderEnvironment::deterministic()).unwrap();
    assert!(!layout.items.is_empty());
    assert!(rendered.svg().contains("font-family:Configured,sans-serif"));
    assert!(rendered.svg().contains("font-size:22px"));
}

#[allow(dead_code)]
mod common;
