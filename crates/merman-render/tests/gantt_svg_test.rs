use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, GradientStop,
    InsetsPx, LinearGradient, OrdinalSelector, PatternKind, PatternSpec, Specified,
    ThemeColorValue, ThemeGeometryPatch, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
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
use merman_render::LayoutOptions;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

fn gantt_task_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    gantt_task_rule_theme(ThemeRule::new(
        ThemeTarget::Task,
        ThemeStylePatch::default().with_fill(fill),
    ))
}

fn gantt_task_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    gantt_task_rule_theme(ThemeRule::new(
        ThemeTarget::Task,
        ThemeStylePatch::default().with_stroke(stroke),
    ))
}

fn gantt_warning_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    gantt_task_rule_theme(
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default().with_stroke(stroke),
        )
        .with_variant(ThemeVariant::Warning),
    )
}

fn gantt_title_fill_theme(fill: CanvasPaint, variant: Option<ThemeVariant>) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(fill),
    );
    gantt_task_rule_theme(match variant {
        Some(variant) => rule.with_variant(variant),
        None => rule,
    })
}

fn gantt_text_fill_theme(fill: CanvasPaint, variant: Option<ThemeVariant>) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(fill),
    );
    gantt_task_rule_theme(match variant {
        Some(variant) => rule.with_variant(variant),
        None => rule,
    })
}

fn gantt_font_stack_theme(font_stack: FontStack) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::GANTT,
                ThemeTextStyle::default().with_font_stack(font_stack),
            ),
        ))
        .expect("compile Gantt font stack theme")
}

fn prepare_gantt_family_with_theme(
    text: &str,
    theme: &DiagramTheme,
) -> family::FamilyRenderArtifact {
    prepare_gantt_family_with_theme_and_engine(text, theme, Engine::new())
}

fn prepare_gantt_family_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::FamilyRenderArtifact {
    try_prepare_gantt_family_with_theme_and_engine(text, theme, engine)
        .expect("prepare themed Gantt artifact")
}

fn try_prepare_gantt_family_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> merman_render::Result<family::FamilyRenderArtifact> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
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

fn style_property<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case(property))
        .map(|(_, value)| value.trim())
        .next_back()
}

fn gantt_task_terminal_inline_paints(svg: &str, property: &str) -> Vec<(String, String, String)> {
    let document = roxmltree::Document::parse(svg).expect("valid themed Gantt SVG XML");
    document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().next() == Some("task"))
        })
        .filter_map(|node| {
            let id = node.attribute("id")?;
            let class = node.attribute("class")?;
            let paint = node
                .attribute("style")
                .and_then(|style| style_property(style, property))?;
            Some((id.to_string(), class.to_string(), paint.to_string()))
        })
        .collect()
}

fn gantt_task_terminal_inline_fills(svg: &str) -> Vec<(String, String, String)> {
    gantt_task_terminal_inline_paints(svg, "fill")
}

fn gantt_task_terminal_inline_strokes(svg: &str) -> Vec<(String, String, String)> {
    gantt_task_terminal_inline_paints(svg, "stroke")
}

const GANTT_TASK_FILL_SOURCE: &str = r#"gantt
dateFormat YYYY-MM-DD
section Delivery
Default: default-task, 2024-01-01, 1d
Active: active, active-task, 2024-01-02, 1d
Done: done, done-task, 2024-01-03, 1d
Critical: crit, crit-task, 2024-01-04, 1d
Active critical: crit, active, active-crit-task, 2024-01-05, 1d
Done critical: crit, done, done-crit-task, 2024-01-06, 1d
Active milestone: milestone, active, active-milestone-task, 2024-01-07, 1d
Done vertical marker: vert, done, done-vert-task, 2024-01-08, 0d
"#;

const GANTT_TASK_VARIANT_TERMINALS: [(ThemeVariant, &[&str]); 4] = [
    (ThemeVariant::Default, &["gantt-config-default-task"]),
    (
        ThemeVariant::Active,
        &[
            "gantt-config-active-task",
            "gantt-config-active-crit-task",
            "gantt-config-active-milestone-task",
        ],
    ),
    (
        ThemeVariant::Success,
        &[
            "gantt-config-done-task",
            "gantt-config-done-crit-task",
            "gantt-config-done-vert-task",
        ],
    ),
    (ThemeVariant::Error, &["gantt-config-crit-task"]),
];

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
fn gantt_static_task_fill_reaches_every_terminal_state_and_shape() {
    for (fill, expected_fill) in [
        (
            CanvasPaint::solid("#123456").expect("valid Gantt task fill"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = gantt_task_fill_theme(fill);
        let artifact = prepare_gantt_family_with_theme(GANTT_TASK_FILL_SOURCE, &theme);
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render strict portable Gantt task fill");
        let terminals = gantt_task_terminal_inline_fills(rendered.svg());

        assert_eq!(terminals.len(), 8, "unexpected Gantt task terminal domain");
        for (id, class, actual_fill) in &terminals {
            assert_eq!(actual_fill, expected_fill, "incorrect fill for {id}");
            let expected_state = match id.as_str() {
                "gantt-config-default-task" => "task0",
                "gantt-config-active-task" => "active0",
                "gantt-config-done-task" => "done0",
                "gantt-config-crit-task" => "crit0",
                "gantt-config-active-crit-task" => "activeCrit0",
                "gantt-config-done-crit-task" => "doneCrit0",
                "gantt-config-active-milestone-task" => "active0",
                "gantt-config-done-vert-task" => "done0",
                _ => panic!("unexpected Gantt task rect {id}"),
            };
            assert!(
                class
                    .split_ascii_whitespace()
                    .any(|token| token == expected_state),
                "Gantt task state class drifted for {id}: {class}"
            );
        }
        assert!(
            terminals
                .iter()
                .find(|(id, _, _)| id == "gantt-config-active-milestone-task")
                .is_some_and(|(_, class, _)| class
                    .split_ascii_whitespace()
                    .any(|c| c == "milestone")),
            "milestone geometry must retain the Active task fill state"
        );
        assert!(
            terminals
                .iter()
                .find(|(id, _, _)| id == "gantt-config-done-vert-task")
                .is_some_and(|(_, class, _)| class.split_ascii_whitespace().any(|c| c == "vert")),
            "vertical marker geometry must retain the Done task fill state"
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
fn gantt_static_task_stroke_reaches_every_terminal_state_and_shape() {
    for (stroke, expected_stroke) in [
        (
            CanvasPaint::solid("#654321").expect("valid Gantt task stroke"),
            "#654321",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = gantt_task_stroke_theme(stroke);
        let artifact = prepare_gantt_family_with_theme(GANTT_TASK_FILL_SOURCE, &theme);
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render strict portable Gantt task stroke");
        let terminals = gantt_task_terminal_inline_strokes(rendered.svg());

        assert_eq!(terminals.len(), 8, "unexpected Gantt task terminal domain");
        for (id, class, actual_stroke) in &terminals {
            assert_eq!(actual_stroke, expected_stroke, "incorrect stroke for {id}");
            let expected_state = match id.as_str() {
                "gantt-config-default-task" => "task0",
                "gantt-config-active-task" => "active0",
                "gantt-config-done-task" => "done0",
                "gantt-config-crit-task" => "crit0",
                "gantt-config-active-crit-task" => "activeCrit0",
                "gantt-config-done-crit-task" => "doneCrit0",
                "gantt-config-active-milestone-task" => "active0",
                "gantt-config-done-vert-task" => "done0",
                _ => panic!("unexpected Gantt task rect {id}"),
            };
            assert!(
                class
                    .split_ascii_whitespace()
                    .any(|token| token == expected_state),
                "Gantt task state class drifted for {id}: {class}"
            );
        }

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
fn gantt_final_mermaid_stroke_keys_own_their_states_independently() {
    let theme = gantt_task_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid source-precedence Gantt task stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "taskBorderColor": "#111111",
            "activeTaskBorderColor": "#222222",
            "doneTaskBorderColor": "#333333",
            "critBorderColor": "#444444"
        }
    })));
    let artifact =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render source-owned Gantt task strokes");
    let terminals = gantt_task_terminal_inline_strokes(rendered.svg());

    for (id, _, stroke) in &terminals {
        let expected = match id.as_str() {
            "gantt-config-default-task" => "#111111",
            "gantt-config-active-task"
            | "gantt-config-active-crit-task"
            | "gantt-config-active-milestone-task" => "#222222",
            "gantt-config-done-task"
            | "gantt-config-done-crit-task"
            | "gantt-config-done-vert-task" => "#333333",
            "gantt-config-crit-task" => "#444444",
            _ => panic!("unexpected Gantt task rect {id}"),
        };
        assert_eq!(stroke, expected, "incorrect final-key owner for {id}");
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_warning_stroke_reaches_today_and_vertical_terminals() {
    let source = "gantt\ndateFormat YYYY-MM-DD\ntodayMarker 2024-01-03\nsection Delivery\nVertical marker: vert, vertical-marker, 2024-01-02, 0d\nTask: regular-task, 2024-01-02, 1d\n";
    let theme = gantt_warning_stroke_theme(
        CanvasPaint::solid("#d97706").expect("valid Gantt warning stroke"),
    );
    let artifact = prepare_gantt_family_with_theme(source, &theme);
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-warning".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render typed Gantt warning stroke");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Gantt warning SVG");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Gantt warning stylesheet");
    assert!(
        stylesheet.contains("#gantt-warning .today{fill:none;stroke:#d97706;stroke-width:2px;}")
    );
    assert!(stylesheet.contains(
        "#gantt-warning .vert{stroke:#d97706;}#gantt-warning .vertText{font-size:15px;text-anchor:middle;fill:#d97706!important;}"
    ));

    let today = document
        .descendants()
        .find(|node| {
            node.has_tag_name("line")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "today")
                })
        })
        .expect("today line terminal");
    assert!(!today
        .attribute("style")
        .is_some_and(|style| style.contains("stroke:")));

    let vertical_task = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("id") == Some("gantt-warning-vertical-marker")
        })
        .expect("vertical task terminal");
    assert_eq!(
        style_property(
            vertical_task.attribute("style").unwrap_or_default(),
            "stroke"
        ),
        Some("#d97706")
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node.attribute("class").is_some_and(|class| {
                class
                    .split_ascii_whitespace()
                    .any(|token| token == "vertText")
            })
    }));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_warning_stroke_is_not_applicable_without_warning_terminals() {
    let source = "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nTask: regular-task, 2024-01-02, 1d\n";
    let theme = gantt_warning_stroke_theme(
        CanvasPaint::solid("#d97706").expect("valid Gantt warning stroke"),
    );
    let artifact = prepare_gantt_family_with_theme(source, &theme);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Gantt without warning terminals");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_warning_stroke_preserves_the_ordinary_task_stroke_for_non_vertical_tasks() {
    let source = "gantt\ndateFormat YYYY-MM-DD\ntodayMarker 2024-01-03\nsection Delivery\nVertical marker: vert, vertical-marker, 2024-01-02, 0d\nTask: regular-task, 2024-01-02, 1d\n";
    let theme = gantt_task_rules_theme([
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#123456").expect("valid task stroke")),
        ),
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#d97706").expect("valid warning stroke")),
        )
        .with_variant(ThemeVariant::Warning),
    ]);
    let rendered = prepare_gantt_family_with_theme(source, &theme)
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-warning-mixed".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render mixed Gantt task and warning strokes");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid mixed Gantt SVG");
    let vertical = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("id") == Some("gantt-warning-mixed-vertical-marker")
        })
        .expect("vertical marker terminal");
    let regular = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("id") == Some("gantt-warning-mixed-regular-task")
        })
        .expect("regular task terminal");
    assert_eq!(
        style_property(vertical.attribute("style").unwrap_or_default(), "stroke"),
        Some("#d97706")
    );
    assert_eq!(
        style_property(regular.attribute("style").unwrap_or_default(), "stroke"),
        Some("#123456")
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_warning_stroke_does_not_require_ordinary_stroke_on_vertical_only_diagrams() {
    let source = "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nVertical marker: vert, vertical-marker, 2024-01-02, 0d\n";
    let theme = gantt_task_rules_theme([
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#123456").expect("valid task stroke")),
        ),
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#d97706").expect("valid warning stroke")),
        )
        .with_variant(ThemeVariant::Warning),
    ]);
    let rendered = prepare_gantt_family_with_theme(source, &theme)
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-warning-vertical-only".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render vertical-only warning Gantt");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid vertical-only SVG");
    let vertical = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("id") == Some("gantt-warning-vertical-only-vertical-marker")
        })
        .expect("vertical marker terminal");
    assert_eq!(
        style_property(vertical.attribute("style").unwrap_or_default(), "stroke"),
        Some("#d97706")
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_warning_stroke_respects_source_owned_vertical_line_color() {
    let source = "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nVertical marker: vert, vertical-marker, 2024-01-02, 0d\n";
    let theme =
        gantt_warning_stroke_theme(CanvasPaint::solid("#d97706").expect("valid warning stroke"));
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {"vertLineColor": "#15803d"}
    })));
    let rendered = prepare_gantt_family_with_theme_and_engine(source, &theme, engine)
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-warning-source-owned".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render source-owned vertical warning Gantt");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid source-owned SVG");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("source-owned warning stylesheet");
    assert!(stylesheet.contains(
        "#gantt-warning-source-owned .vert{stroke:#15803d;}#gantt-warning-source-owned .vertText{font-size:15px;text-anchor:middle;fill:#15803d!important;}"
    ));
    let vertical = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("id") == Some("gantt-warning-source-owned-vertical-marker")
        })
        .expect("source-owned vertical terminal");
    assert!(!vertical
        .attribute("style")
        .is_some_and(|style| style_property(style, "stroke").is_some()));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_warning_stroke_detects_source_owned_comma_delimited_today_marker() {
    let source = "gantt\ndateFormat YYYY-MM-DD\ntodayMarker STROKE : #15803d,opacity:0.5\nsection Delivery\nTask: regular-task, 2024-01-02, 1d\n";
    let theme =
        gantt_warning_stroke_theme(CanvasPaint::solid("#d97706").expect("valid warning stroke"));
    let rendered = prepare_gantt_family_with_theme(source, &theme)
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-warning-comma-marker".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render comma-delimited source-owned today marker");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid comma-marker SVG");
    let today = document
        .descendants()
        .find(|node| node.has_tag_name("line") && node.attribute("class") == Some("today"))
        .expect("today marker terminal");
    assert_eq!(
        style_property(today.attribute("style").unwrap_or_default(), "stroke"),
        Some("#15803d")
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_task_stroke_stays_bound_to_semantic_state_after_date_sorting() {
    let source = concat!(
        "gantt\n",
        "dateFormat YYYY-MM-DD\n",
        "section Delivery\n",
        "Late plain: late-task, 2024-01-03, 1d\n",
        "Early active: active, early-task, 2024-01-01, 1d\n",
    );
    let theme = gantt_task_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid semantic Gantt task stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "activeTaskBorderColor": "#abcdef" }
    })));
    let artifact = prepare_gantt_family_with_theme_and_engine(source, &theme, engine);
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render sorted Gantt task strokes");
    let terminals = gantt_task_terminal_inline_strokes(rendered.svg());
    let by_id = terminals
        .into_iter()
        .map(|(id, _, stroke)| (id, stroke))
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(
        by_id.get("gantt-config-late-task").map(String::as_str),
        Some("#654321")
    );
    assert_eq!(
        by_id.get("gantt-config-early-task").map(String::as_str),
        Some("#abcdef")
    );
}

#[test]
fn gantt_axis_tick_lines_retain_current_color_fallback() {
    let svg = render_gantt_svg_from_text_with_theme(
        GANTT_TASK_FILL_SOURCE,
        &gantt_task_fill_theme(CanvasPaint::Transparent),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed Gantt SVG XML");
    let tick_lines = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node.ancestors().any(|ancestor| {
                    ancestor.has_tag_name("g") && ancestor.attribute("class") == Some("tick")
                })
        })
        .collect::<Vec<_>>();
    assert!(
        !tick_lines.is_empty(),
        "Gantt fixture must emit axis tick lines"
    );
    assert!(
        tick_lines
            .iter()
            .all(|line| line.attribute("stroke") == Some("currentColor")),
        "axis tick lines must retain Mermaid's currentColor fallback"
    );
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("the themed Gantt SVG must include its scoped stylesheet");
    assert!(
        stylesheet.contains(".grid .tick{stroke:"),
        "the scoped stylesheet must remain the Gantt grid stroke owner"
    );
}

#[test]
fn gantt_typed_font_stack_reaches_scoped_css_and_strict_receipt() {
    let font_stack =
        FontStack::new(["Gantt Typed", "monospace"]).expect("valid Gantt typed font stack");
    let expected_font = font_stack.as_css();
    let source =
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTyped: typed-task, 2024-01-01, 1d";
    let svg = render_gantt_svg_from_text_with_theme(source, &gantt_font_stack_theme(font_stack));
    let document = roxmltree::Document::parse(&svg).expect("valid typed Gantt SVG XML");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("typed Gantt SVG must include its stylesheet");

    assert!(
        stylesheet.contains(&format!("#gantt-config{{font-family:{expected_font};")),
        "typed Gantt font stack must be emitted by the scoped stylesheet"
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("text")
                && node.attribute("id") == Some("gantt-config-typed-task-text")
        }),
        "typed Gantt task label must survive the strict terminal receipt"
    );
}

#[test]
fn gantt_title_fill_reaches_section_and_diagram_title_css() {
    let source = "gantt\ntitle Theme title\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d";
    for variant in [None, Some(ThemeVariant::Default)] {
        let svg = render_gantt_svg_from_text_with_theme(
            source,
            &gantt_title_fill_theme(
                CanvasPaint::solid("#123456").expect("valid title fill"),
                variant,
            ),
        );
        let document = roxmltree::Document::parse(&svg).expect("valid themed Gantt SVG XML");
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("themed Gantt SVG must include its stylesheet");
        assert!(
            stylesheet.contains(
                "#gantt-config .sectionTitle0{fill:#123456;}#gantt-config .sectionTitle1{fill:#123456;}#gantt-config .sectionTitle2{fill:#123456;}#gantt-config .sectionTitle3{fill:#123456;}"
            ),
            "section title CSS must consume typed Title.fill: {stylesheet}"
        );
        assert!(
            stylesheet.contains(
                "#gantt-config .titleText{text-anchor:middle;font-size:18px;fill:#123456;font-family:"
            ),
            "diagram title CSS must consume typed Title.fill: {stylesheet}"
        );
        assert!(
            document.descendants().any(|node| {
                node.has_tag_name("text")
                    && node.attribute("class") == Some("titleText")
                    && node.text() == Some("Theme title")
            }),
            "typed Gantt title route must retain the visible title terminal"
        );
    }
}

#[test]
fn gantt_title_fill_requires_a_visible_title_terminal() {
    let theme = gantt_title_fill_theme(
        CanvasPaint::solid("#123456").expect("valid title fill"),
        None,
    );
    let rendered = prepare_gantt_family_with_theme(
        "gantt\ndateFormat YYYY-MM-DD\nTask: task, 2024-01-01, 1d",
        &theme,
    )
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some("gantt-config".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
    .expect("render Gantt without visible title terminal");

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_text_fill_reaches_grid_and_ordinary_task_label_css() {
    let text_fill = "#123456";
    let svg = render_gantt_svg_from_text_with_theme(
        GANTT_TASK_FILL_SOURCE,
        &gantt_text_fill_theme(
            CanvasPaint::solid(text_fill).expect("valid Gantt text fill"),
            None,
        ),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed Gantt SVG XML");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("themed Gantt SVG must include its stylesheet");

    let grid_rule = stylesheet
        .split("#gantt-config .grid .tick text{")
        .nth(1)
        .and_then(|rule| rule.split('}').next())
        .expect("Gantt grid text selector");
    assert!(grid_rule.contains(&format!("fill:{text_fill};")));
    let task_rule = stylesheet
        .split("#gantt-config .taskText0,#gantt-config .taskText1,#gantt-config .taskText2,#gantt-config .taskText3{")
        .nth(1)
        .and_then(|rule| rule.split('}').next())
        .expect("Gantt task text selector");
    assert!(task_rule.contains(&format!("fill:{text_fill};")));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node.attribute("id") == Some("gantt-config-default-task-text")
            && node.text() == Some("Default")
    }));

    let completion = prepare_gantt_family_with_theme(
        GANTT_TASK_FILL_SOURCE,
        &gantt_text_fill_theme(
            CanvasPaint::solid(text_fill).expect("valid Gantt text fill"),
            None,
        ),
    )
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some("gantt-config".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
    .expect("render typed Gantt text fill")
    .into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_text_fill_preserves_independent_source_ownership() {
    let typed_fill = "#123456";
    let theme = gantt_text_fill_theme(
        CanvasPaint::solid(typed_fill).expect("valid mixed Gantt text fill"),
        Some(ThemeVariant::Default),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "textColor": "#abcdef" }
    })));
    let rendered =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine)
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render mixed source-owned Gantt text fill");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid mixed Gantt SVG XML");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("mixed Gantt SVG must include its stylesheet");

    let grid_rule = stylesheet
        .split("#gantt-config .grid .tick text{")
        .nth(1)
        .and_then(|rule| rule.split('}').next())
        .expect("Gantt grid text selector");
    assert!(grid_rule.contains("fill:#abcdef;"));
    let task_rule = stylesheet
        .split("#gantt-config .taskText0,#gantt-config .taskText1,#gantt-config .taskText2,#gantt-config .taskText3{")
        .nth(1)
        .and_then(|rule| rule.split('}').next())
        .expect("Gantt task text selector");
    assert!(task_rule.contains(&format!("fill:{typed_fill};")));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_text_fill_preserves_task_text_source_ownership() {
    let typed_fill = "#123456";
    let theme = gantt_text_fill_theme(
        CanvasPaint::solid(typed_fill).expect("valid mixed Gantt text fill"),
        Some(ThemeVariant::Default),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "taskTextColor": "#fedcba" }
    })));
    let rendered =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine)
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render task-text source-owned Gantt text fill");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Gantt SVG XML");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Gantt SVG must include its stylesheet");

    let grid_rule = stylesheet
        .split("#gantt-config .grid .tick text{")
        .nth(1)
        .and_then(|rule| rule.split('}').next())
        .expect("Gantt grid text selector");
    assert!(grid_rule.contains(&format!("fill:{typed_fill};")));
    let task_rule = stylesheet
        .split("#gantt-config .taskText0,#gantt-config .taskText1,#gantt-config .taskText2,#gantt-config .taskText3{")
        .nth(1)
        .and_then(|rule| rule.split('}').next())
        .expect("Gantt task text selector");
    assert!(task_rule.contains("fill:#fedcba;"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_text_fill_is_not_applicable_when_both_source_tokens_own_terminals() {
    let theme = gantt_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid source-owned Gantt text fill"),
        Some(ThemeVariant::Default),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "textColor": "#abcdef",
            "taskTextColor": "#fedcba"
        }
    })));
    let rendered =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine)
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render fully source-owned Gantt text fill");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_site_font_family_outranks_typed_font_stack() {
    let typed_stack = FontStack::single("Gantt Typed").expect("valid Gantt typed font stack");
    let theme = gantt_font_stack_theme(typed_stack);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "gantt": {"fontFamily": "Site Gantt Font"}
    })));
    let svg = prepare_gantt_family_with_theme_and_engine(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d",
        &theme,
        engine,
    )
    .render_svg(
        &SvgRenderOptions {
            diagram_id: Some("gantt-config".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
    .expect("render site-owned Gantt font stack")
    .svg()
    .to_owned();
    let document = roxmltree::Document::parse(&svg).expect("valid site-owned Gantt SVG XML");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("site-owned Gantt SVG must include its stylesheet");

    assert!(stylesheet.contains("#gantt-config{font-family:Site Gantt Font;"));
    assert!(!stylesheet.contains("Gantt Typed"));
}

#[test]
fn gantt_final_mermaid_fill_keys_own_their_states_independently() {
    let theme = gantt_task_fill_theme(
        CanvasPaint::solid("#123456").expect("valid source-precedence Gantt task fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "taskBkgColor": "#111111",
            "activeTaskBkgColor": "#222222",
            "doneTaskBkgColor": "#333333",
            "critBkgColor": "#444444"
        }
    })));
    let artifact =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render source-owned Gantt task fills");
    let terminals = gantt_task_terminal_inline_fills(rendered.svg());

    for (id, _, fill) in &terminals {
        let expected = match id.as_str() {
            "gantt-config-default-task" => "#111111",
            "gantt-config-active-task"
            | "gantt-config-active-crit-task"
            | "gantt-config-active-milestone-task" => "#222222",
            "gantt-config-done-task"
            | "gantt-config-done-crit-task"
            | "gantt-config-done-vert-task" => "#333333",
            "gantt-config-crit-task" => "#444444",
            _ => panic!("unexpected Gantt task rect {id}"),
        };
        assert_eq!(fill, expected, "incorrect final-key owner for {id}");
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_each_final_mermaid_fill_key_suppresses_only_its_terminal_states() {
    let theme = gantt_task_fill_theme(
        CanvasPaint::solid("#123456").expect("valid partial-precedence Gantt task fill"),
    );
    for (key, source_fill, source_owned_ids) in [
        (
            "taskBkgColor",
            "#111111",
            &["gantt-config-default-task"][..],
        ),
        (
            "activeTaskBkgColor",
            "#222222",
            &[
                "gantt-config-active-task",
                "gantt-config-active-crit-task",
                "gantt-config-active-milestone-task",
            ][..],
        ),
        (
            "doneTaskBkgColor",
            "#333333",
            &[
                "gantt-config-done-task",
                "gantt-config-done-crit-task",
                "gantt-config-done-vert-task",
            ][..],
        ),
        ("critBkgColor", "#444444", &["gantt-config-crit-task"][..]),
    ] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(
            serde_json::json!({ "themeVariables": { (key): source_fill } }),
        ));
        let artifact =
            prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render partially source-owned Gantt task fills");

        for (id, _, fill) in gantt_task_terminal_inline_fills(rendered.svg()) {
            let expected = if source_owned_ids.contains(&id.as_str()) {
                source_fill
            } else {
                "#123456"
            };
            assert_eq!(fill, expected, "{key} suppressed the wrong terminal {id}");
        }

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
fn gantt_task_fill_is_not_applicable_without_task_rects() {
    let theme = gantt_task_fill_theme(
        CanvasPaint::solid("#123456").expect("valid empty-domain Gantt task fill"),
    );
    let artifact = prepare_gantt_family_with_theme("gantt\ndateFormat YYYY-MM-DD\n", &theme);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render empty Gantt task domain");
    assert!(gantt_task_terminal_inline_fills(rendered.svg()).is_empty());

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_task_fill_supports_variant_and_rejects_ordinal_clear_gradient_and_pattern_routes() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#123456").expect("valid Gantt gradient start"),
            )
            .expect("valid Gantt gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#abcdef").expect("valid Gantt gradient end"),
            )
            .expect("valid Gantt gradient stop"),
        ],
    )
    .expect("valid Gantt gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#123456").expect("valid Gantt pattern color"),
    )
    .expect("valid Gantt pattern");
    let mut clear = ThemeStylePatch::default();
    clear.paint.fill = Specified::Clear;
    let solid = || {
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").expect("valid Gantt solid fill"))
    };
    let unsupported_cases = [
        ThemeRule::new(ThemeTarget::Task, solid())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid Gantt task ordinal")),
        ThemeRule::new(ThemeTarget::Task, clear),
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
        ),
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default().with_fill(CanvasPaint::Pattern(pattern)),
        ),
    ];

    for (variant, expected_ids) in GANTT_TASK_VARIANT_TERMINALS {
        let theme =
            gantt_task_rule_theme(ThemeRule::new(ThemeTarget::Task, solid()).with_variant(variant));
        let artifact = prepare_gantt_family_with_theme(GANTT_TASK_FILL_SOURCE, &theme);
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render typed Gantt task fill variant");
        let terminals = gantt_task_terminal_inline_fills(rendered.svg());
        let actual_ids = terminals
            .iter()
            .filter(|(_, _, fill)| fill == "#123456")
            .map(|(id, _, _)| id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            actual_ids, expected_ids,
            "unexpected terminal fill scope for {variant:?}"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }

    for rule in unsupported_cases {
        let theme = gantt_task_rule_theme(rule);
        let result = try_prepare_gantt_family_with_theme_and_engine(
            GANTT_TASK_FILL_SOURCE,
            &theme,
            Engine::new(),
        )
        .and_then(|artifact| {
            artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        });
        let error = match result {
            Ok(_) => panic!("unsupported Gantt task fill routes must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::GANTT, 1))
        );
    }
}

#[test]
fn gantt_task_stroke_supports_variant_and_rejects_ordinal_clear_gradient_and_pattern_routes() {
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(
                0.0,
                ThemeColorValue::parse("#123456").expect("valid Gantt gradient start"),
            )
            .expect("valid Gantt gradient stop"),
            GradientStop::new(
                1.0,
                ThemeColorValue::parse("#abcdef").expect("valid Gantt gradient end"),
            )
            .expect("valid Gantt gradient stop"),
        ],
    )
    .expect("valid Gantt gradient");
    let pattern = PatternSpec::new(
        PatternKind::Grid,
        8.0,
        8.0,
        ThemeColorValue::parse("#123456").expect("valid Gantt pattern color"),
    )
    .expect("valid Gantt pattern");
    let mut clear = ThemeStylePatch::default();
    clear.stroke.paint = Specified::Clear;
    let solid = || {
        ThemeStylePatch::default()
            .with_stroke(CanvasPaint::solid("#654321").expect("valid Gantt solid stroke"))
    };
    let unsupported_cases = [
        ThemeRule::new(ThemeTarget::Task, solid())
            .with_ordinal(OrdinalSelector::exact(1).expect("valid Gantt task ordinal")),
        ThemeRule::new(ThemeTarget::Task, clear),
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default().with_stroke(CanvasPaint::LinearGradient(gradient)),
        ),
        ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default().with_stroke(CanvasPaint::Pattern(pattern)),
        ),
    ];

    for (variant, expected_ids) in GANTT_TASK_VARIANT_TERMINALS {
        let theme =
            gantt_task_rule_theme(ThemeRule::new(ThemeTarget::Task, solid()).with_variant(variant));
        let artifact = prepare_gantt_family_with_theme(GANTT_TASK_FILL_SOURCE, &theme);
        let rendered = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render typed Gantt task stroke variant");
        let terminals = gantt_task_terminal_inline_strokes(rendered.svg());
        let actual_ids = terminals
            .iter()
            .filter(|(_, _, stroke)| stroke == "#654321")
            .map(|(id, _, _)| id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            actual_ids, expected_ids,
            "unexpected terminal stroke scope for {variant:?}"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }

    for rule in unsupported_cases {
        let theme = gantt_task_rule_theme(rule);
        let result = try_prepare_gantt_family_with_theme_and_engine(
            GANTT_TASK_FILL_SOURCE,
            &theme,
            Engine::new(),
        )
        .and_then(|artifact| {
            artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        });
        let error = match result {
            Ok(_) => panic!("unsupported Gantt task stroke routes must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((merman_render::DiagramFamilyId::GANTT, 1))
        );
    }
}

#[test]
fn gantt_task_stroke_is_not_applicable_without_task_rects() {
    let theme = gantt_task_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid empty-domain Gantt task stroke"),
    );
    let artifact = prepare_gantt_family_with_theme("gantt\ndateFormat YYYY-MM-DD\n", &theme);
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render empty Gantt task domain");
    assert!(gantt_task_terminal_inline_strokes(rendered.svg()).is_empty());

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
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
