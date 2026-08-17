use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, InsetsPx, OrdinalSelector,
    Specified, ThemeGeometryPatch, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::KanbanDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

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
fn kanban_task_radius_rejects_qualified_and_mixed_rules() {
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

    let mixed_legacy = ThemeRule::new(
        ThemeTarget::Task,
        radius_style.with_stroke(CanvasPaint::solid("#123456").expect("valid task stroke")),
    );
    let error = try_render_kanban_svg_with_theme(source, &kanban_task_rules_theme([mixed_legacy]))
        .expect_err("a mixed direct and legacy Kanban rule must not be signed Applied");
    assert!(matches!(
        error,
        merman_render::Error::LegacyFamilyThemeCompatibility {
            family_id: merman_render::DiagramFamilyId::KANBAN,
            ..
        }
    ));
}

#[test]
fn kanban_task_radius_is_not_applicable_without_items() {
    render_kanban_svg_with_theme(
        "kanban\n  todo[Todo]\n",
        &kanban_task_radius_theme(Specified::Value(8.0)),
    );
}
