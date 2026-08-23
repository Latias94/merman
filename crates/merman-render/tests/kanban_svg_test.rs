use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, InsetsPx,
    MermaidThemeCompatibility, OrdinalPalette, OrdinalSelector, Specified, ThemeColorValue,
    ThemeGeometryPatch, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::KanbanDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions};

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
            .map(|color| ThemeColorValue::parse(*color).expect("valid Kanban task palette color")),
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

fn render_kanban_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> family::RenderedFamilySvg {
    try_render_kanban_with_theme_and_engine(source, theme, engine, portability)
        .expect("render themed Kanban")
}

fn try_render_kanban_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Kanban")
        .expect("detect themed Kanban");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Kanban session");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Kanban")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("kanban-palette".to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
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
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Kanban")
        .expect("detect themed Kanban");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Kanban session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    render_kanban_artifact(artifact)
}

fn render_kanban_svg_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> (KanbanDiagramLayout, String) {
    try_render_kanban_svg_with_theme(source, theme).expect("render themed Kanban SVG")
}

fn parse_layout_and_render(source: &str) -> (KanbanDiagramLayout, String) {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Kanban")
        .expect("detect Kanban");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("start deterministic render session");
    let artifact =
        family::prepare(parsed, &LayoutOptions::default(), session).expect("prepare Kanban layout");
    render_kanban_artifact(artifact).expect("render Kanban SVG")
}

fn try_render_kanban_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse resource-bound Kanban")
        .expect("detect resource-bound Kanban");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("start resource-bound Kanban render session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
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
            Engine::new(),
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
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }
}

#[test]
fn kanban_static_task_fill_respects_the_mermaid_background_owner() {
    let theme =
        kanban_task_fill_theme(CanvasPaint::solid("#123456").expect("valid Kanban task fill"));
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "themeVariables": { "background": "#fedcba" }
        }))),
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
            Engine::new(),
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
        assert_eq!(evidence.compatibility_residual_count(), 0);
    }
}

#[test]
fn kanban_static_task_stroke_respects_the_mermaid_node_border_owner() {
    let rendered = render_kanban_with_theme_and_engine(
        "kanban\n  todo[Todo]\n    task[Task]\n",
        &kanban_task_stroke_theme(CanvasPaint::solid("#654321").expect("valid Kanban task stroke")),
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "themeVariables": { "nodeBorder": "#fedcba" }
        }))),
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
        Engine::new(),
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
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "darkMode": true
        }))),
        ThemePortabilityRequirement::RequirePortable,
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid dark Kanban SVG");

    assert_eq!(
        kanban_task_rect_style(&document, "kanban-palette-task"),
        "fill:hsl(210, 65.3846153846%, 10.3921568627%);"
    );
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
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let native = rendered
        .finalize_resvg(&SvgPipeline::resvg_safe())
        .expect("finalize Kanban task-label foreground for resvg");
    let native_svg = merman_render::__private::native_export_svg(native.svg());
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
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "themeVariables": { "background": "#fedcba" }
        }))),
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
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "secure": []
        }))),
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
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "textColor": "#fedcba" }
            }))),
            "kanban\n  todo[Todo]\n    task[Task]\n",
        ),
        (
            "source",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
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
        Engine::new(),
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
            Engine::new().with_site_config(MermaidConfig::from_value(site_config)),
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
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "background": "#fedcba" }
            }))),
            plain_source,
        ),
        (
            "source background",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
            concat!(
                "%%{init: {\"themeVariables\": {\"background\": \"#fedcba\"}}}%%\n",
                "kanban\n  todo[Todo]\n    task[Task]\n",
            ),
        ),
        (
            "site primaryTextColor",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "primaryTextColor": "#fedcba" }
            }))),
            plain_source,
        ),
        (
            "source primaryTextColor",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
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
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "textColor": "#fedcba" }
            }))),
            plain_source,
        ),
        (
            "source textColor",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
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
        Engine::new(),
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
        Engine::new(),
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
        Engine::new(),
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
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);

    let error = match try_render_kanban_with_theme_and_engine(
        source,
        &theme,
        Engine::new(),
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
fn kanban_typed_icon_task_label_preserves_the_label_background_class() {
    let theme = kanban_task_label_theme("#123456");
    let rendered = render_kanban_with_theme_and_engine(
        concat!(
            "kanban\n  todo[Todo]\n",
            "    task@{ icon: star, label: 'Task' }\n",
        ),
        &theme,
        Engine::new(),
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
        Engine::new(),
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
        Engine::new(),
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
        Engine::new(),
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
fn kanban_task_radius_is_not_applicable_without_items() {
    render_kanban_svg_with_theme(
        "kanban\n  todo[Todo]\n",
        &kanban_task_radius_theme(Specified::Value(8.0)),
    );
}
