use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, InsetsPx, OrdinalSelector, Specified,
    ThemeGeometryPatch, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeVariant,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::GanttDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn layout_gantt_from_text(text: &str) -> GanttDiagramLayout {
    layout_gantt_from_text_at_container_width(text, LayoutOptions::default().container_width)
}

fn layout_gantt_from_text_at_container_width(
    text: &str,
    container_width: f64,
) -> GanttDiagramLayout {
    let environment = RenderEnvironment::deterministic();
    layout_gantt_from_text_with_environment(text, container_width, &environment)
}

fn layout_gantt_from_text_with_environment(
    text: &str,
    container_width: f64,
    environment: &RenderEnvironment,
) -> GanttDiagramLayout {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let mut options = LayoutOptions::default();
    options.container_width = container_width;
    let session = environment.begin_session().expect("render session");
    let artifact = family::prepare(parsed, &options, session).expect("layout ok");
    let projection = artifact.layout_json().expect("Gantt layout projection");
    serde_json::from_value(projection["layout"]["GanttDiagram"].clone()).expect("Gantt layout")
}

fn gantt_task_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Gantt task theme")
}

fn gantt_task_rule_theme(rule: ThemeRule) -> DiagramTheme {
    gantt_task_rules_theme([rule])
}

fn gantt_task_radius_theme(radius: Specified<f32>) -> DiagramTheme {
    gantt_task_rule_theme(ThemeRule::new(
        ThemeTarget::Task,
        ThemeStylePatch {
            geometry: ThemeGeometryPatch { radius },
            ..ThemeStylePatch::default()
        },
    ))
}

fn prepare_gantt_family_with_theme(
    text: &str,
    theme: &DiagramTheme,
) -> family::FamilyRenderArtifact {
    let parsed = merman_render::__private::install_parse_compatibility(theme, Engine::new())
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse themed Gantt diagram")
        .expect("detect themed Gantt diagram");
    let session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_core::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Gantt session");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Gantt artifact")
}

fn render_gantt_svg_from_text_with_theme(text: &str, theme: &DiagramTheme) -> String {
    prepare_gantt_family_with_theme(text, theme)
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Gantt SVG")
        .svg()
        .to_owned()
}

#[test]
fn gantt_layout_uses_the_operation_container_width_unless_config_overrides_it() {
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: 2024-01-01, 1d";
    let narrow = layout_gantt_from_text_at_container_width(source, 640.0);
    let wide = layout_gantt_from_text_at_container_width(source, 960.0);

    assert_eq!(narrow.width, 640.0);
    assert_eq!(wide.width, 960.0);

    let configured = layout_gantt_from_text_at_container_width(
        "---\nconfig:\n  gantt:\n    useWidth: 420\n---\ngantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: 2024-01-01, 1d",
        960.0,
    );
    assert_eq!(configured.width, 420.0);
}

#[test]
fn gantt_static_task_radius_reaches_layout_svg_and_portable_evidence() {
    let source = r#"gantt
dateFormat YYYY-MM-DD
section Delivery
Radius task: radius-task, 2024-01-01, 1d
"#;

    let baseline = render_gantt_svg_from_text(source);
    let empty_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile empty typed theme");
    assert_eq!(
        render_gantt_svg_from_text_with_theme(source, &empty_theme).as_bytes(),
        baseline.as_bytes(),
        "an absent Gantt task radius must preserve default SVG bytes"
    );

    let theme = gantt_task_radius_theme(Specified::Value(7.0));
    let artifact = prepare_gantt_family_with_theme(source, &theme);
    let projection = artifact
        .layout_json()
        .expect("themed Gantt layout projection");
    let layout: GanttDiagramLayout =
        serde_json::from_value(projection["layout"]["GanttDiagram"].clone())
            .expect("themed Gantt layout");
    let task = layout
        .tasks
        .iter()
        .find(|task| task.id == "radius-task")
        .expect("themed Gantt task");
    assert_eq!(task.bar.rx, 7.0);
    assert_eq!(task.bar.ry, 7.0);

    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Gantt SVG")
        .svg()
        .to_owned();
    let document = roxmltree::Document::parse(&svg).expect("valid themed Gantt SVG XML");
    let task_rect = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("id") == Some("gantt-config-radius-task")
        })
        .expect("themed Gantt task rect");
    assert_eq!(task_rect.attribute("rx"), Some("7"));
    assert_eq!(task_rect.attribute("ry"), Some("7"));
}

#[test]
fn gantt_task_radius_clear_restores_the_mermaid_baseline() {
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d";
    let svg =
        render_gantt_svg_from_text_with_theme(source, &gantt_task_radius_theme(Specified::Clear));
    let document = roxmltree::Document::parse(&svg).expect("valid cleared Gantt SVG XML");
    let task = document
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("id") == Some("gantt-config-task"))
        .expect("cleared Gantt task rect");
    assert_eq!(task.attribute("rx"), Some("3"));
    assert_eq!(task.attribute("ry"), Some("3"));
}

#[test]
fn gantt_task_radius_rejects_unsupported_selector_and_mixed_geometry() {
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d";
    let radius_style = ThemeStylePatch {
        geometry: ThemeGeometryPatch {
            radius: Specified::Value(7.0),
        },
        ..ThemeStylePatch::default()
    };
    let rules = [
        ThemeRule::new(ThemeTarget::Task, radius_style.clone()).with_variant(ThemeVariant::Default),
        ThemeRule::new(ThemeTarget::Task, radius_style.clone())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid task ordinal")),
        ThemeRule::new(
            ThemeTarget::Task,
            radius_style.with_padding(InsetsPx::all(4.0)),
        ),
    ];

    for rule in rules {
        let theme = gantt_task_rule_theme(rule);
        let error = match prepare_gantt_family_with_theme(source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        {
            Ok(_) => panic!("unsupported Gantt task theme routes must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::GANTT, 1))
        );
    }
}

#[test]
fn gantt_task_radius_is_not_applicable_without_task_bars() {
    render_gantt_svg_from_text_with_theme(
        "gantt\ndateFormat YYYY-MM-DD\n",
        &gantt_task_radius_theme(Specified::Value(7.0)),
    );
}

#[test]
fn gantt_task_variant_radius_routes_fail_closed_for_matching_task_states() {
    for (task_tag, variant) in [
        ("active", ThemeVariant::Active),
        ("done", ThemeVariant::Success),
        ("crit", ThemeVariant::Error),
    ] {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: {task_tag}, task, 2024-01-01, 1d"
        );
        let theme = gantt_task_rule_theme(
            ThemeRule::new(
                ThemeTarget::Task,
                ThemeStylePatch {
                    geometry: ThemeGeometryPatch {
                        radius: Specified::Value(11.0),
                    },
                    ..ThemeStylePatch::default()
                },
            )
            .with_variant(variant),
        );

        let error = prepare_gantt_family_with_theme(&source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .err()
            .expect("a matching unsupported task variant must remain residual");
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::GANTT, 1))
        );
    }
}

#[test]
fn gantt_task_variant_override_does_not_reuse_unqualified_geometry() {
    let source = concat!(
        "gantt\n",
        "dateFormat YYYY-MM-DD\n",
        "section Delivery\n",
        "Plain: plain, 2024-01-01, 1d\n",
        "Active: active, active-task, 2024-01-02, 1d\n",
    );
    let radius_style = |radius| ThemeStylePatch {
        geometry: ThemeGeometryPatch {
            radius: Specified::Value(radius),
        },
        ..ThemeStylePatch::default()
    };
    let theme = gantt_task_rules_theme([
        ThemeRule::new(ThemeTarget::Task, radius_style(7.0)),
        ThemeRule::new(ThemeTarget::Task, radius_style(11.0)).with_variant(ThemeVariant::Active),
    ]);
    let artifact = prepare_gantt_family_with_theme(source, &theme);
    let projection = artifact.layout_json().expect("Gantt layout projection");
    let layout: GanttDiagramLayout =
        serde_json::from_value(projection["layout"]["GanttDiagram"].clone())
            .expect("themed Gantt layout");
    assert_eq!(layout.tasks[0].bar.rx, 7.0);
    assert_eq!(layout.tasks[1].bar.rx, 3.0);

    let error = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .err()
        .expect("the unsupported active override must fail strict completion");
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::GANTT, 1))
    );
}

#[test]
fn gantt_task_radius_stays_bound_to_semantic_occurrence_after_date_sorting() {
    let source = concat!(
        "gantt\n",
        "dateFormat YYYY-MM-DD\n",
        "section Delivery\n",
        "Late plain: late-task, 2024-01-03, 1d\n",
        "Early active critical: crit, active, early-task, 2024-01-01, 1d\n",
    );
    let radius_style = |radius| ThemeStylePatch {
        geometry: ThemeGeometryPatch {
            radius: Specified::Value(radius),
        },
        ..ThemeStylePatch::default()
    };
    let theme = gantt_task_rules_theme([
        ThemeRule::new(ThemeTarget::Task, radius_style(7.0)),
        ThemeRule::new(ThemeTarget::Task, radius_style(11.0)).with_variant(ThemeVariant::Active),
    ]);
    let artifact = prepare_gantt_family_with_theme(source, &theme);
    let projection = artifact.layout_json().expect("Gantt layout projection");
    let layout: GanttDiagramLayout =
        serde_json::from_value(projection["layout"]["GanttDiagram"].clone())
            .expect("themed Gantt layout");

    assert_eq!(layout.tasks[0].id, "early-task");
    assert_eq!(layout.tasks[0].bar.rx, 3.0);
    assert_eq!(layout.tasks[1].id, "late-task");
    assert_eq!(layout.tasks[1].bar.rx, 7.0);

    let error = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .err()
        .expect("the unsupported active override must remain residual");
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::GANTT, 1))
    );
}

#[test]
fn gantt_task_radius_receipt_binds_the_canonical_svg_number() {
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d";
    let svg = render_gantt_svg_from_text_with_theme(
        source,
        &gantt_task_radius_theme(Specified::Value(5.0e-10)),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed Gantt SVG XML");
    let task = document
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("id") == Some("gantt-config-task"))
        .expect("Gantt task rect");
    assert_eq!(task.attribute("rx"), Some("0"));
    assert_eq!(task.attribute("ry"), Some("0"));
}

struct RawBBoxProbeMeasurer {
    calls: Arc<AtomicUsize>,
    width: f64,
}

impl TextMeasurer for RawBBoxProbeMeasurer {
    fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
        panic!("Gantt task labels must use the raw SVG text bbox operation")
    }

    fn measure_svg_raw_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.width
    }
}

#[test]
fn gantt_task_labels_route_through_raw_svg_bbox_measurement() {
    let calls = Arc::new(AtomicUsize::new(0));
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-raw-bbox").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(RawBBoxProbeMeasurer {
            calls: Arc::clone(&calls),
            width: 200.0,
        }),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let layout = layout_gantt_from_text_with_environment(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d",
        1_184.0,
        &environment,
    );

    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(layout.tasks[0].label.width, 200.0);
}

#[test]
fn gantt_label_placement_uses_the_resolved_container_edges() {
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.gantt-raw-bbox-placement").unwrap(),
            "v1",
        )
        .unwrap(),
        Arc::new(RawBBoxProbeMeasurer {
            calls: Arc::new(AtomicUsize::new(0)),
            width: 200.0,
        }),
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let layout = layout_gantt_from_text_with_environment(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nFull range: full, 2024-01-01, 10d\nStart label: start, 2024-01-01, 1d\nEnd label: end, 2024-01-10, 1d",
        1_184.0,
        &environment,
    );
    let start = layout
        .tasks
        .iter()
        .find(|task| task.id == "start")
        .expect("start task");
    let end = layout
        .tasks
        .iter()
        .find(|task| task.id == "end")
        .expect("end task");

    assert!(start.label.class.contains("taskTextOutsideRight"));
    assert!(start.label.x > start.bar.x + start.bar.width);
    assert!(end.label.class.contains("taskTextOutsideLeft"));
    assert_eq!(end.label.x, end.bar.x - 5.0);
}

#[test]
fn gantt_layout_stops_at_the_maximum_utc_date_without_panicking() {
    // The raw model boundary case lives beside the private layout entry point.
}

fn render_gantt_svg_from_text(text: &str) -> String {
    render_gantt_svg_from_text_with_engine(Engine::new(), text)
}

fn render_gantt_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    let session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_core::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .begin_session()
        .expect("begin render session");
    let parsed = futures::executor::block_on(
        engine.parse_diagram_for_render_model(text, ParseOptions::default()),
    )
    .expect("parse ok")
    .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render svg")
        .svg()
        .to_owned()
}

fn try_render_gantt_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_core::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Gantt resource-bound session");
    let parsed = futures::executor::block_on(
        Engine::new().parse_diagram_for_render_model(text, ParseOptions::default()),
    )
    .expect("parse Gantt resource-bound fixture")
    .expect("detect Gantt resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("gantt-bounded".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn gantt_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"gantt
dateFormat YYYY-MM-DD
todayMarker off
section Delivery
Plan: plan, 2024-01-01, 2d
Ship: ship, after plan, 1d
"#;
    let baseline = try_render_gantt_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Gantt baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Gantt fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Gantt SVG byte ceiling");
    let exact = try_render_gantt_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Gantt family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Gantt SVG byte ceiling");
    let error = try_render_gantt_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Gantt family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Gantt MaxSvgBytes rejection, got {error}");
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
fn gantt_task_text_height_follows_final_svg_security_sanitization() {
    let source = r#"gantt
dateFormat YYYY-MM-DD
section Delivery
Task: task, 2024-01-01, 1d
"#;
    let strict = render_gantt_svg_from_text(source);
    let loose = render_gantt_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        source,
    );

    let task_text_heights = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid Gantt SVG XML");
        document
            .descendants()
            .filter(|node| node.has_tag_name("text") && node.attribute("id").is_some())
            .filter_map(|node| node.attribute("text-height").map(str::to_owned))
            .collect::<Vec<_>>()
    };

    assert!(task_text_heights(&strict).is_empty(), "{strict}");
    assert_eq!(task_text_heights(&loose), vec!["20"], "{loose}");
}

#[test]
fn gantt_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_gantt_svg_from_text(
        r#"---
title: Frontmatter schedule
---
gantt
dateFormat YYYY-MM-DD
section Delivery
Task: 2024-01-01, 1d
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="titleText">Frontmatter schedule</text>"#),
        "frontmatter title should render when the Gantt body has none: {frontmatter_svg}"
    );

    let body_svg = render_gantt_svg_from_text(
        r#"---
title: Frontmatter schedule
---
gantt
title Body schedule
dateFormat YYYY-MM-DD
section Delivery
Task: 2024-01-01, 1d
"#,
    );
    assert!(body_svg.contains(r#"class="titleText">Body schedule</text>"#));
    assert!(!body_svg.contains(">Frontmatter schedule</text>"));
}

#[test]
fn gantt_explicit_whitespace_title_overrides_frontmatter_without_trimming() {
    let svg = render_gantt_svg_from_text(concat!(
        "---\n",
        "title: Frontmatter schedule\n",
        "---\n",
        "gantt\n",
        "title  \n",
        "dateFormat YYYY-MM-DD\n",
        "section Delivery\n",
        "Task: 2024-01-01, 1d\n",
    ));

    assert!(
        svg.contains(r#"class="titleText"> </text>"#),
        "the one remaining Jison separator must be rendered exactly: {svg}"
    );
    assert!(!svg.contains(">Frontmatter schedule</text>"));
}

#[test]
fn gantt_svg_frontmatter_config_fields_affect_visible_output() {
    let svg = render_gantt_svg_from_text(
        r#"---
displayMode: compact
config:
  gantt:
    useWidth: 420
    rightPadding: 10
    topAxis: true
    numberSectionStyles: 2
---
gantt
  title Config Frontmatter SVG Fields
  dateFormat YYYY-MM-DD
  axisFormat %Y-%m-%d
  tickInterval 1day
  todayMarker off
  section Alpha
  Task A :a1, 2024-01-01, 1d
  section Beta
  Task B :b1, 2024-01-02, 1d
"#,
    );

    assert!(
        svg.contains(r#"viewBox="0 0 420 "#)
            && svg.contains(r#"style="max-width: 420px; background-color: white;""#),
        "frontmatter gantt.useWidth should set rendered SVG width: {svg}"
    );
    assert_eq!(
        svg.matches(r#"<g class="grid" transform="translate(75, 50)""#)
            .count(),
        1,
        "frontmatter gantt.topAxis should add the top axis grid at top padding: {svg}"
    );
    assert_eq!(
        svg.matches(r#"<g class="grid" transform="translate(75, "#)
            .count(),
        2,
        "frontmatter gantt.topAxis should render both top and bottom axes: {svg}"
    );
    assert!(
        svg.contains(r#"width="415" height="24" class="section section0""#)
            && svg.contains(r#"width="415" height="24" class="section section1""#),
        "frontmatter gantt.rightPadding and numberSectionStyles should affect visible rows: {svg}"
    );
    assert!(
        svg.contains(r#"class="sectionTitle sectionTitle0""#)
            && svg.contains(r#"class="sectionTitle sectionTitle1""#)
            && svg.contains(r#"id="gantt-config-a1""#)
            && svg.contains(r#"id="gantt-config-b1-text""#),
        "configured Gantt SVG should expose section classes and scoped task DOM: {svg}"
    );
}

#[test]
fn gantt_vertical_markers_do_not_affect_standard_row_layout() {
    let layout = layout_gantt_from_text(
        r#"
gantt
dateFormat YYYY-MM-DD
section Delivery
Start marker: vert,marker-start,2024-01-01,0d
Task A: task-a,2024-01-02,1d
Middle marker: vert,marker-middle,2024-01-05,0d
Task B: task-b,2024-01-06,1d
Final marker: vert,marker-final,2024-01-10,0d
"#,
    );
    assert_eq!(layout.height, 148.0);
    assert_eq!(
        layout.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        vec![0, 1]
    );

    let markers = layout
        .tasks
        .iter()
        .filter(|task| task.vert)
        .collect::<Vec<_>>();
    assert_eq!(markers.len(), 3);
    assert!(markers.iter().all(|task| task.order == -1));
    assert!(markers.iter().all(|task| task.bar.height == 88.0));
    assert!(markers.iter().all(|task| task.label.y == 143.0));

    let final_marker = markers
        .iter()
        .find(|task| task.id == "marker-final")
        .expect("final marker");
    assert_eq!(
        final_marker.bar.x,
        layout.width - layout.right_padding,
        "vertical markers must remain part of the time domain"
    );
}

#[test]
fn gantt_vertical_markers_do_not_affect_compact_row_packing() {
    let layout = layout_gantt_from_text(
        r#"---
displayMode: compact
---
gantt
dateFormat YYYY-MM-DD
section Delivery
Long marker: vert,marker-long,2024-01-01,31d
Task A: task-a,2024-01-01,1d
Task B: task-b,2024-01-03,1d
"#,
    );
    assert_eq!(layout.height, 124.0);
    assert_eq!(
        layout.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(
        layout
            .tasks
            .iter()
            .map(|task| (task.id.as_str(), task.order))
            .collect::<Vec<_>>(),
        vec![("marker-long", -1), ("task-a", 0), ("task-b", 0)]
    );
}
