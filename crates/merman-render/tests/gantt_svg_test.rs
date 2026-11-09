type FamilyFixture = (
    merman_model::presentation::PresentationDiagram,
    merman_render::LayoutOptions,
    merman_render::environment::RenderSession,
);

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
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
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let mut options = LayoutOptions::default();
    options.container_width = container_width;
    let session = environment.begin_session().expect("render session");
    let artifact = family::prepare(&parsed, &options, session).expect("layout ok");
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

#[test]
fn gantt_global_paint_does_not_certify_unsupported_sibling_facets() {
    let source = "gantt\ntitle Release audit\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nShip: task, 2024-01-01, 4d\n";
    for target in [ThemeTarget::Title, ThemeTarget::Text] {
        for clear in [false, true] {
            for separate_rule in [false, true] {
                let paint =
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#c2188b").unwrap());
                let mut typography = ThemeStylePatch::default();
                typography.typography.font_size_px = if clear {
                    Specified::Clear
                } else {
                    Specified::Value(31.0)
                };
                let rules = if separate_rule {
                    vec![
                        ThemeRule::new(target, paint),
                        ThemeRule::new(target, typography),
                    ]
                } else {
                    vec![ThemeRule::new(
                        target,
                        ThemeStylePatch {
                            typography: typography.typography,
                            ..paint
                        },
                    )]
                };
                let theme = gantt_task_rules_theme(rules);
                let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                for portability in [
                    ThemePortabilityRequirement::BestEffort,
                    ThemePortabilityRequirement::RequirePortable,
                ] {
                    let session = RenderEnvironment::deterministic()
                        .with_theme_portability_requirement(portability)
                        .begin_session_with_theme(&theme)
                        .unwrap();
                    let result = family::prepare(&parsed, &LayoutOptions::default(), session)
                        .unwrap()
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
                    if portability == ThemePortabilityRequirement::RequirePortable {
                        assert_eq!(
                            result
                                .err()
                                .expect("unsupported typography must reject strict completion")
                                .unverified_family_theme(),
                            Some((merman_render::DiagramFamilyId::GANTT, 1))
                        );
                    } else {
                        let rendered = result.unwrap();
                        assert!(rendered.svg().contains("#c2188b"));
                        assert!(!rendered.svg().contains("font-size:31px"));
                        let evidence = merman_render::__private::family_evidence(
                            rendered.into_completion().report(),
                        );
                        assert_eq!(evidence.required_count(), if separate_rule { 2 } else { 1 });
                        assert_eq!(evidence.applied_count(), usize::from(separate_rule));
                        assert_eq!(evidence.theme_residual_count(), 1);
                        assert_eq!(evidence.accounted_count(), evidence.required_count());
                    }
                }
            }
        }
    }
}

#[test]
fn gantt_unsupported_title_styles_require_visible_title_terminals() {
    for (heading, section_font_size, visible) in [
        ("", 11, false),
        ("title Release audit\n", 11, true),
        ("section Delivery\n", 11, true),
        ("section Delivery\n", 0, false),
        ("title Release audit\nsection Delivery\n", 0, true),
    ] {
        let config = serde_json::json!({"gantt": {"sectionFontSize": section_font_size}});
        let source = format!(
            "---\nconfig: {config}\n---\ngantt\ndateFormat YYYY-MM-DD\ntodayMarker off\n{heading}Ship: task, 2024-01-01, 4d\n"
        );
        for clear in [false, true] {
            for (with_fill, split) in [(false, false), (true, false), (true, true)] {
                let mut typography = ThemeStylePatch::default();
                typography.typography.font_size_px = if clear {
                    Specified::Clear
                } else {
                    Specified::Value(31.0)
                };
                let mut rules = vec![ThemeRule::new(ThemeTarget::Title, typography.clone())];
                if with_fill {
                    let paint = ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#c2188b").unwrap());
                    rules = if split {
                        vec![
                            ThemeRule::new(ThemeTarget::Title, paint),
                            ThemeRule::new(ThemeTarget::Title, typography),
                        ]
                    } else {
                        vec![ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch {
                                typography: typography.typography,
                                ..paint
                            },
                        )]
                    };
                }
                let required = rules.len();
                let theme = gantt_task_rules_theme(rules);
                let (parsed, options, _) = prepare_gantt_family_with_theme(&source, &theme);
                for portability in [
                    ThemePortabilityRequirement::BestEffort,
                    ThemePortabilityRequirement::RequirePortable,
                ] {
                    let session = RenderEnvironment::deterministic()
                        .with_theme_portability_requirement(portability)
                        .begin_session_with_theme(&theme)
                        .unwrap();
                    let result = family::prepare(&parsed, &options, session)
                        .unwrap()
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
                    if visible && portability == ThemePortabilityRequirement::RequirePortable {
                        assert_eq!(
                            result.err().unwrap().unverified_family_theme(),
                            Some((merman_render::DiagramFamilyId::GANTT, 1))
                        );
                    } else {
                        let evidence = merman_render::__private::family_evidence(
                            result.unwrap().into_completion().report(),
                        );
                        assert_eq!(evidence.required_count(), required);
                        assert_eq!(evidence.accounted_count(), required);
                        assert_eq!(evidence.theme_residual_count(), usize::from(visible));
                        assert_eq!(
                            evidence.not_applicable_count(),
                            if visible { 0 } else { required }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn gantt_task_label_typography_requires_its_own_visible_winning_terminal() {
    for (case, font_size, task, variant, visible) in [
        ("zero", 0, "Ship: a, 2024-01-02, 1d\n", None, false),
        ("positive", 11, "Ship: a, 2024-01-02, 1d\n", None, true),
        (
            "vertical",
            0,
            "Marker: vert, v, 2024-01-02, 0d\n",
            None,
            true,
        ),
        (
            "unrelated-vertical",
            0,
            "Ship: a, 2024-01-02, 1d\nMarker: active, vert, v, 2024-01-02, 0d\n",
            Some(ThemeVariant::Default),
            false,
        ),
    ] {
        let config = serde_json::json!({
            "gantt": {"fontSize": font_size, "sectionFontSize": 0, "tickInterval": "1day"}
        });
        let source = format!(
            "---\nconfig: {config}\n---\ngantt\ntitle Visible diagram title\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\n{task}"
        );
        for clear in [false, true] {
            for (with_fill, split) in [(false, false), (true, false), (true, true)] {
                let mut typography = ThemeStylePatch::default();
                typography.typography.font_size_px = if clear {
                    Specified::Clear
                } else {
                    Specified::Value(31.0)
                };
                let paint =
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#c2188b").unwrap());
                let patches = match (with_fill, split) {
                    (false, _) => vec![typography],
                    (true, true) => vec![paint, typography],
                    (true, false) => vec![ThemeStylePatch {
                        typography: typography.typography,
                        ..paint
                    }],
                };
                let rules = patches
                    .into_iter()
                    .map(|patch| {
                        let rule = ThemeRule::new(ThemeTarget::TaskLabel, patch);
                        match variant {
                            Some(variant) => rule.with_variant(variant),
                            None => rule,
                        }
                    })
                    .collect::<Vec<_>>();
                let required = rules.len();
                let theme = gantt_task_rules_theme(rules);
                let (parsed, options, _) = prepare_gantt_family_with_theme(&source, &theme);
                for portability in [
                    ThemePortabilityRequirement::BestEffort,
                    ThemePortabilityRequirement::RequirePortable,
                ] {
                    let session = RenderEnvironment::deterministic()
                        .with_theme_portability_requirement(portability)
                        .begin_session_with_theme(&theme)
                        .unwrap();
                    let result = family::prepare(&parsed, &options, session)
                        .unwrap()
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
                    if visible && portability == ThemePortabilityRequirement::RequirePortable {
                        assert_eq!(
                            result.err().unwrap().unverified_family_theme(),
                            Some((merman_render::DiagramFamilyId::GANTT, 1)),
                            "{case}/clear={clear}/split={split}"
                        );
                        continue;
                    }
                    let rendered = result.unwrap();
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    assert!(
                        document
                            .descendants()
                            .any(|node| { node.attribute("class") == Some("tick") }),
                        "axis text must not prove a TaskLabel rule"
                    );
                    assert!(document.descendants().any(|node| {
                        node.attribute("class") == Some("titleText")
                            && node.text() == Some("Visible diagram title")
                    }));
                    assert!(!rendered.svg().contains("font-size:31px"));
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(evidence.required_count(), required, "{case}");
                    assert_eq!(evidence.accounted_count(), required, "{case}");
                    assert_eq!(
                        evidence.theme_residual_count(),
                        usize::from(visible),
                        "{case}"
                    );
                    assert_eq!(
                        evidence.applied_count(),
                        usize::from(visible && split),
                        "{case}"
                    );
                    assert_eq!(
                        evidence.not_applicable_count(),
                        if visible { 0 } else { required },
                        "{case}"
                    );
                }
            }
        }
    }
}

#[test]
fn gantt_font_ownership_follows_the_consumed_configuration_value() {
    let theme = gantt_font_stack_theme(FontStack::single("Courier New").unwrap());
    let diagram = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nShip: task, 2024-01-01, 4d\n";
    for source_owned in [false, true] {
        for (config, expected, applied) in [
            (
                serde_json::json!({"gantt": {"fontFamily": "  "}}),
                "\"Courier New\"",
                1,
            ),
            (
                serde_json::json!({"gantt": {"fontFamily": 32}}),
                "\"Courier New\"",
                1,
            ),
            (
                serde_json::json!({"gantt": {"fontFamily": ["Georgia"]}}),
                "\"Courier New\"",
                1,
            ),
            (
                serde_json::json!({"gantt": {"fontFamily": "Georgia"}, "themeVariables": {"fontFamily": "Verdana"}, "fontFamily": "Arial"}),
                "Georgia",
                0,
            ),
            (
                serde_json::json!({"gantt": {"fontFamily": 32}, "themeVariables": {"fontFamily": "Georgia"}}),
                "Georgia",
                0,
            ),
        ] {
            let (source, engine) = if source_owned {
                (
                    format!("---\nconfig: {config}\n---\n{diagram}"),
                    MermaidEngine::new(),
                )
            } else {
                (
                    diagram.to_owned(),
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                )
            };
            let (parsed, options, session) =
                prepare_gantt_family_with_theme_and_engine(&source, &theme, engine);
            let rendered = family::prepare(&parsed, &options, session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            assert!(
                rendered.svg().contains(&format!("font-family:{expected};")),
                "{}",
                rendered.svg()
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), applied);
            assert_eq!(evidence.not_applicable_count(), 1 - applied);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
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

fn prepare_gantt_family_with_theme(text: &str, theme: &DiagramTheme) -> FamilyFixture {
    prepare_gantt_family_with_theme_and_engine(text, theme, MermaidEngine::new())
}

fn prepare_gantt_family_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
) -> FamilyFixture {
    try_prepare_gantt_family_with_theme_and_engine(text, theme, engine)
}

fn try_prepare_gantt_family_with_theme_and_engine(
    text: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
) -> FamilyFixture {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse themed Gantt diagram")
        .expect("detect themed Gantt diagram")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_model::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Gantt session");
    (parsed, LayoutOptions::default(), session)
}

fn render_gantt_svg_from_text_with_theme(text: &str, theme: &DiagramTheme) -> String {
    let (fixture_parsed_1, fixture_layout_1, fixture_session_1) =
        prepare_gantt_family_with_theme(text, theme);
    family::prepare(&fixture_parsed_1, &fixture_layout_1, fixture_session_1)
        .expect("prepare family fixture")
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

fn render_gantt_preset(source: &str, preset: merman_render::diagram_theme::ThemePreset) -> String {
    let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
        .begin_session_with_theme(&theme)
        .unwrap();
    family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_owned()
}

#[test]
fn gantt_editor_presets_preserve_task_states_and_critical_strokes() {
    use merman_render::diagram_theme::ThemePreset;
    for (preset, default, active, done, critical) in [
        (
            ThemePreset::EditorLight,
            "#f8fafc",
            "#2563eb",
            "#e2e8f0",
            "#be123c",
        ),
        (
            ThemePreset::EditorDark,
            "#111827",
            "#60a5fa",
            "#1f2937",
            "#fb7185",
        ),
    ] {
        let svg = render_gantt_preset(GANTT_TASK_FILL_SOURCE, preset);
        let fills = gantt_task_terminal_inline_fills(&svg);
        for (id, expected) in [
            ("default-task", default),
            ("active-task", active),
            ("done-task", done),
            ("crit-task", critical),
            ("active-crit-task", active),
            ("done-crit-task", done),
            ("active-milestone-task", active),
            ("done-vert-task", done),
        ] {
            assert_eq!(
                fills
                    .iter()
                    .find(|(actual, _, _)| actual == &format!("gantt-config-{id}"))
                    .map(|(_, _, fill)| fill.as_str()),
                Some(expected),
                "{preset:?} {id}"
            );
        }
        let strokes = gantt_task_terminal_inline_strokes(&svg);
        for id in ["crit-task", "active-crit-task", "done-crit-task"] {
            assert_eq!(
                strokes
                    .iter()
                    .find(|(actual, _, _)| actual == &format!("gantt-config-{id}"))
                    .map(|(_, _, stroke)| stroke.as_str()),
                Some(critical),
                "{preset:?} {id} critical border"
            );
        }
    }
}

#[test]
fn gantt_editor_presets_bind_foregrounds_to_inside_outside_and_clickable_labels() {
    use merman_render::diagram_theme::ThemePreset;
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nActive: active, active-task, 2024-01-01, 100d\nDone: done, done-task, 2024-01-01, 100d\nCritical: crit, crit-task, 2024-01-01, 100d\nA very long outside active label: active, outside-task, 2024-01-01, 1d\nclick active-task href \"https://example.com\"\n";
    for (preset, active_label, text) in [
        (ThemePreset::EditorLight, "#ffffff", "#0f172a"),
        (ThemePreset::EditorDark, "#0f172a", "#e5e7eb"),
    ] {
        let svg = render_gantt_preset(source, preset);
        let doc = roxmltree::Document::parse(&svg).unwrap();
        for (id, expected) in [
            ("active-task-text", active_label),
            ("done-task-text", text),
            ("crit-task-text", active_label),
            ("outside-task-text", text),
        ] {
            let label = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("text")
                        && node.attribute("id") == Some(format!("gantt-config-{id}").as_str())
                })
                .expect("task label terminal");
            assert_eq!(
                label
                    .attribute("style")
                    .and_then(|style| style_property(style, "fill")),
                Some(format!("{expected}!important").as_str()),
                "{preset:?}: {label:?}"
            );
        }
    }
}

#[test]
fn gantt_preset_labels_preserve_explicit_terminal_foreground_variables() {
    use merman_render::diagram_theme::ThemePreset;
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nDefault: default-task, 2024-01-01, 100d\nActive: active, active-task, 2024-01-01, 100d\nDone: done, done-task, 2024-01-01, 100d\nA very long outside done label: done, outside-task, 2024-01-01, 1d\nclick default-task href \"https://example.com\"\n";
    for (variable, terminal) in [
        ("taskTextDarkColor", "active-task-text"),
        ("taskTextClickableColor", "default-task-text"),
        ("taskTextOutsideColor", "outside-task-text"),
    ] {
        let configured =
            format!("---\nconfig:\n  themeVariables:\n    {variable}: '#123456'\n---\n{source}");
        let svg = render_gantt_preset(&configured, ThemePreset::EditorDark);
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let terminal_id = format!("gantt-config-{terminal}");
        let label = doc
            .descendants()
            .find(|node| {
                node.has_tag_name("text") && node.attribute("id") == Some(terminal_id.as_str())
            })
            .unwrap();
        assert_eq!(
            label
                .attribute("style")
                .and_then(|style| style_property(style, "fill")),
            None,
            "{variable} must own {terminal}"
        );
        assert!(
            doc.descendants()
                .filter(|node| node.has_tag_name("style"))
                .filter_map(|node| node.text())
                .any(|css| css.contains("fill:#123456;")),
            "explicit {variable} must reach final CSS"
        );
    }
}

#[test]
fn gantt_generic_text_reaches_active_and_done_visible_labels() {
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nActive: active, active-task, 2024-01-01, 100d\nDone: done, done-task, 2024-01-01, 100d";
    for variant in [None, Some(ThemeVariant::Default)] {
        let svg = render_gantt_svg_from_text_with_theme(
            source,
            &gantt_text_fill_theme(CanvasPaint::solid("#123456").unwrap(), variant),
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        for id in [
            "gantt-config-active-task-text",
            "gantt-config-done-task-text",
        ] {
            let label = doc
                .descendants()
                .find(|node| node.has_tag_name("text") && node.attribute("id") == Some(id))
                .unwrap();
            assert_eq!(
                label
                    .attribute("style")
                    .and_then(|style| style_property(style, "fill")),
                Some("#123456!important")
            );
        }
    }
}

#[test]
fn gantt_mixed_labels_preserve_vertical_fallback_and_final_active_paint() {
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Work\nMarker :vert, marker, 2026-09-01, 0d\nActive task :active, active, 2026-09-02, 1d";
    let theme = gantt_task_rule_theme(
        ThemeRule::new(
            ThemeTarget::TaskLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
        .with_variant(ThemeVariant::Active),
    );
    let svg = render_gantt_svg_from_text_with_theme(source, &theme);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let css = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect::<String>();
    assert!(css.contains(".vertText{font-size:15px;text-anchor:middle;fill:navy;}"));
    let marker = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "vertText")
                })
        })
        .unwrap();
    assert_eq!(marker.attribute("style"), None);
    let active = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "activeText0")
                })
        })
        .unwrap();
    assert_eq!(
        active
            .attribute("style")
            .and_then(|style| style_property(style, "fill")),
        Some("#123456!important")
    );
}

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
    let (fixture_parsed_2, fixture_layout_2, fixture_session_2) =
        prepare_gantt_family_with_theme(source, &theme);
    let artifact = family::prepare(&fixture_parsed_2, &fixture_layout_2, fixture_session_2)
        .expect("prepare family fixture");
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
        let (fixture_parsed_3, fixture_layout_3, fixture_session_3) =
            prepare_gantt_family_with_theme(GANTT_TASK_FILL_SOURCE, &theme);
        let artifact = family::prepare(&fixture_parsed_3, &fixture_layout_3, fixture_session_3)
            .expect("prepare family fixture");
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
        let (fixture_parsed_4, fixture_layout_4, fixture_session_4) =
            prepare_gantt_family_with_theme(GANTT_TASK_FILL_SOURCE, &theme);
        let artifact = family::prepare(&fixture_parsed_4, &fixture_layout_4, fixture_session_4)
            .expect("prepare family fixture");
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {
                "taskBorderColor": "#111111",
                "activeTaskBorderColor": "#222222",
                "doneTaskBorderColor": "#333333",
                "critBorderColor": "#444444"
            }
        })),
    );
    let (fixture_parsed_20, fixture_layout_20, fixture_session_20) =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
    let artifact = family::prepare(&fixture_parsed_20, &fixture_layout_20, fixture_session_20)
        .expect("prepare family fixture");
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

    assert_eq!(
        terminals.len(),
        8,
        "all eight source-owned task terminals must exist"
    );
    let actual_ids = terminals
        .iter()
        .map(|(id, _, _)| id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected_ids = [
        "gantt-config-default-task",
        "gantt-config-active-task",
        "gantt-config-done-task",
        "gantt-config-crit-task",
        "gantt-config-active-crit-task",
        "gantt-config-done-crit-task",
        "gantt-config-active-milestone-task",
        "gantt-config-done-vert-task",
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_ids, expected_ids);

    for (id, _, stroke) in &terminals {
        let expected = match id.as_str() {
            "gantt-config-default-task" => "#111111",
            "gantt-config-active-task" | "gantt-config-active-milestone-task" => "#222222",
            "gantt-config-done-task" | "gantt-config-done-vert-task" => "#333333",
            "gantt-config-crit-task"
            | "gantt-config-active-crit-task"
            | "gantt-config-done-crit-task" => "#444444",
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
    let (fixture_parsed_5, fixture_layout_5, fixture_session_5) =
        prepare_gantt_family_with_theme(source, &theme);
    let artifact = family::prepare(&fixture_parsed_5, &fixture_layout_5, fixture_session_5)
        .expect("prepare family fixture");
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
    assert!(
        !today
            .attribute("style")
            .is_some_and(|style| style.contains("stroke:"))
    );

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
    let (fixture_parsed_6, fixture_layout_6, fixture_session_6) =
        prepare_gantt_family_with_theme(source, &theme);
    let artifact = family::prepare(&fixture_parsed_6, &fixture_layout_6, fixture_session_6)
        .expect("prepare family fixture");
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
    let (fixture_parsed_7, fixture_layout_7, fixture_session_7) =
        prepare_gantt_family_with_theme(source, &theme);
    let rendered = family::prepare(&fixture_parsed_7, &fixture_layout_7, fixture_session_7)
        .expect("prepare family fixture")
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
    let (fixture_parsed_8, fixture_layout_8, fixture_session_8) =
        prepare_gantt_family_with_theme(source, &theme);
    let rendered = family::prepare(&fixture_parsed_8, &fixture_layout_8, fixture_session_8)
        .expect("prepare family fixture")
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {"vertLineColor": "#15803d"}
        })),
    );
    let (fixture_parsed_21, fixture_layout_21, fixture_session_21) =
        prepare_gantt_family_with_theme_and_engine(source, &theme, engine);
    let rendered = family::prepare(&fixture_parsed_21, &fixture_layout_21, fixture_session_21)
        .expect("prepare family fixture")
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
    assert!(
        vertical
            .attribute("style")
            .is_none_or(|style| style_property(style, "stroke").is_none())
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
fn gantt_warning_stroke_detects_source_owned_comma_delimited_today_marker() {
    for marker in [
        "STROKE : #15803d,opacity:0.5",
        "opacity:0.5,STROKE : #15803d",
    ] {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker {marker}\nsection Delivery\nTask: regular-task, 2024-01-02, 1d\n"
        );
        let theme = gantt_warning_stroke_theme(
            CanvasPaint::solid("#d97706").expect("valid warning stroke"),
        );
        let (fixture_parsed_9, fixture_layout_9, fixture_session_9) =
            prepare_gantt_family_with_theme(&source, &theme);
        let rendered = family::prepare(&fixture_parsed_9, &fixture_layout_9, fixture_session_9)
            .expect("prepare family fixture")
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
}

#[test]
fn gantt_ordinary_stroke_respects_config_owned_vertical_color() {
    let theme = gantt_task_stroke_theme(CanvasPaint::solid("#654321").unwrap());
    for has_regular in [false, true] {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nVertical: vert, vertical, 2024-01-02, 0d\n{}",
            if has_regular {
                "Regular: regular, 2024-01-02, 1d\n"
            } else {
                ""
            }
        );
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": {"vertLineColor": "#15803d"}
            })),
        );
        let (fixture_parsed_22, fixture_layout_22, fixture_session_22) =
            prepare_gantt_family_with_theme_and_engine(&source, &theme, engine);
        let rendered = family::prepare(&fixture_parsed_22, &fixture_layout_22, fixture_session_22)
            .expect("prepare family fixture")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let vertical = document
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|part| part == "vert")
                    })
            })
            .unwrap();
        assert_eq!(
            style_property(vertical.attribute("style").unwrap_or_default(), "stroke"),
            None
        );
        if has_regular {
            let regular = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.ends_with("-regular"))
                })
                .unwrap();
            assert_eq!(
                style_property(regular.attribute("style").unwrap_or_default(), "stroke"),
                Some("#654321")
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(has_regular));
        assert_eq!(evidence.not_applicable_count(), usize::from(!has_regular));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "activeTaskBorderColor": "#abcdef" }
        })),
    );
    let (fixture_parsed_23, fixture_layout_23, fixture_session_23) =
        prepare_gantt_family_with_theme_and_engine(source, &theme, engine);
    let artifact = family::prepare(&fixture_parsed_23, &fixture_layout_23, fixture_session_23)
        .expect("prepare family fixture");
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
    let (fixture_parsed_10, fixture_layout_10, fixture_session_10) =
        prepare_gantt_family_with_theme(
            "gantt\ndateFormat YYYY-MM-DD\nTask: task, 2024-01-01, 1d",
            &theme,
        );
    let rendered = family::prepare(&fixture_parsed_10, &fixture_layout_10, fixture_session_10)
        .expect("prepare family fixture")
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
fn gantt_zero_size_section_titles_do_not_prove_title_fill() {
    let theme = gantt_title_fill_theme(CanvasPaint::solid("#123456").unwrap(), None);
    for (section_font_size, heading, applied) in
        [(0, "", 0), (11, "", 1), (0, "title Visible title\n", 1)]
    {
        let config = serde_json::json!({"gantt": {"sectionFontSize": section_font_size}});
        let source = format!(
            "---\nconfig: {config}\n---\ngantt\n{heading}dateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nShip: a, 2024-01-01, 1d\n"
        );
        let (parsed, options, session) = prepare_gantt_family_with_theme(&source, &theme);
        let rendered = family::prepare(&parsed, &options, session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let section = document
            .descendants()
            .find(|node| node.attribute("class") == Some("sectionTitle sectionTitle0"))
            .expect("zero-size section text remains in the SVG");
        assert_eq!(
            section.attribute("font-size"),
            Some(section_font_size.to_string().as_str())
        );
        assert_eq!(
            section.first_element_child().unwrap().text(),
            Some("Delivery")
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), 1 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn gantt_zero_size_task_labels_do_not_prove_text_or_task_label_fill() {
    for target in [ThemeTarget::Text, ThemeTarget::TaskLabel] {
        let theme = gantt_task_rule_theme(ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        ));
        for font_size in [0, 11] {
            let config = serde_json::json!({
                "gantt": {"fontSize": font_size, "sectionFontSize": 0},
                "themeVariables": {"textColor": "#abcdef"}
            });
            let source = format!(
                "---\nconfig: {config}\n---\ngantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nShip: a, 2024-01-01, 4d\n"
            );
            let (parsed, options, session) = prepare_gantt_family_with_theme(&source, &theme);
            let rendered = family::prepare(&parsed, &options, session)
                .unwrap()
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some("gantt-config".to_owned()),
                        ..SvgRenderOptions::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let baseline_svg = render_gantt_svg_from_text(&source);
            let baseline = roxmltree::Document::parse(&baseline_svg).unwrap();
            for id in ["gantt-config-a", "gantt-config-a-text"] {
                let terminal = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .unwrap();
                let original = baseline
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .unwrap();
                for attribute in ["x", "y", "width", "height", "font-size", "class"] {
                    assert_eq!(terminal.attribute(attribute), original.attribute(attribute));
                }
                assert_eq!(terminal.text(), original.text());
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), usize::from(font_size != 0));
            assert_eq!(evidence.not_applicable_count(), usize::from(font_size == 0));
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn gantt_zero_size_tasks_preserve_fixed_size_font_consumers() {
    let theme = gantt_font_stack_theme(FontStack::single("monospace").unwrap());
    for (role, heading, marker, tick_interval, applied) in [
        ("none", "", "", "1month", 0),
        ("axis", "", "", "1day", 1),
        ("title", "title Visible title\n", "", "1month", 1),
        (
            "vertical",
            "",
            "Marker: vert, v, 2024-01-02, 0d\n",
            "1month",
            1,
        ),
    ] {
        let config = serde_json::json!({
            "gantt": {"fontSize": 0, "sectionFontSize": 0, "tickInterval": tick_interval}
        });
        let source = format!(
            "---\nconfig: {config}\n---\ngantt\n{heading}dateFormat YYYY-MM-DD\ntodayMarker off\nsection Delivery\nShip: a, 2024-01-02, 1d\n{marker}"
        );
        let (parsed, options, session) = prepare_gantt_family_with_theme(&source, &theme);
        let rendered = family::prepare(&parsed, &options, session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert_eq!(
            rendered
                .metadata()
                .effective_config
                .get_str("gantt.tickInterval"),
            Some(tick_interval),
            "{role}"
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let tick_count = document
            .descendants()
            .filter(|node| node.attribute("class") == Some("tick"))
            .count();
        assert_eq!(tick_count > 0, role == "axis", "{role}");
        if role == "vertical" {
            let label = document
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("Marker"))
                .unwrap();
            assert_eq!(label.attribute("font-size"), Some("0"));
            assert!(
                label
                    .attribute("class")
                    .unwrap()
                    .split_ascii_whitespace()
                    .any(|class| class == "vertText")
            );
            assert!(rendered.svg().contains(".vertText{font-size:15px;"));
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{role}");
        assert_eq!(evidence.applied_count(), applied, "{role}");
        assert_eq!(evidence.not_applicable_count(), 1 - applied, "{role}");
        assert_eq!(evidence.theme_residual_count(), 0, "{role}");
    }
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

    let (fixture_parsed_11, fixture_layout_11, fixture_session_11) =
        prepare_gantt_family_with_theme(
            GANTT_TASK_FILL_SOURCE,
            &gantt_text_fill_theme(
                CanvasPaint::solid(text_fill).expect("valid Gantt text fill"),
                None,
            ),
        );
    let completion = family::prepare(&fixture_parsed_11, &fixture_layout_11, fixture_session_11)
        .expect("prepare family fixture")
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "textColor": "#abcdef" }
        })),
    );
    let (fixture_parsed_24, fixture_layout_24, fixture_session_24) =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
    let rendered = family::prepare(&fixture_parsed_24, &fixture_layout_24, fixture_session_24)
        .expect("prepare family fixture")
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "taskTextColor": "#fedcba" }
        })),
    );
    let (fixture_parsed_25, fixture_layout_25, fixture_session_25) =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
    let rendered = family::prepare(&fixture_parsed_25, &fixture_layout_25, fixture_session_25)
        .expect("prepare family fixture")
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
fn gantt_text_fill_is_not_applicable_when_all_source_roles_own_terminals() {
    let theme = gantt_text_fill_theme(
        CanvasPaint::solid("#123456").expect("valid source-owned Gantt text fill"),
        Some(ThemeVariant::Default),
    );
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {
                "textColor": "#abcdef",
                "taskTextColor": "#fedcba",
                "taskTextDarkColor": "#fedcba",
                "taskTextOutsideColor": "#fedcba",
                "taskTextClickableColor": "#fedcba",
                "vertLineColor": "#fedcba"
            }
        })),
    );
    let (fixture_parsed_26, fixture_layout_26, fixture_session_26) =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
    let rendered = family::prepare(&fixture_parsed_26, &fixture_layout_26, fixture_session_26)
        .expect("prepare family fixture")
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
fn gantt_generic_text_keeps_unowned_active_labels_when_grid_and_ordinary_labels_are_source_owned() {
    let theme = gantt_text_fill_theme(
        CanvasPaint::solid("#123456").unwrap(),
        Some(ThemeVariant::Default),
    );
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "textColor": "#abcdef", "taskTextColor": "#fedcba" }
        })),
    );
    let source = "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nOrdinary: ordinary, 2024-01-01, 100d\nActive: active, active-task, 2024-01-01, 100d\n";
    let (parsed, options, session) =
        prepare_gantt_family_with_theme_and_engine(source, &theme, engine);
    let rendered = family::prepare(&parsed, &options, session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("gantt-config".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
    for (id, expected) in [
        ("gantt-config-ordinary-text", None),
        ("gantt-config-active-task-text", Some("#123456!important")),
    ] {
        let label = doc
            .descendants()
            .find(|node| node.has_tag_name("text") && node.attribute("id") == Some(id))
            .unwrap();
        assert_eq!(
            label
                .attribute("style")
                .and_then(|style| style_property(style, "fill")),
            expected
        );
    }
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn gantt_site_font_family_outranks_typed_font_stack() {
    let typed_stack = FontStack::single("Gantt Typed").expect("valid Gantt typed font stack");
    let theme = gantt_font_stack_theme(typed_stack);
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "gantt": {"fontFamily": "Site Gantt Font"}
        })),
    );
    let (fixture_parsed_27, fixture_layout_27, fixture_session_27) =
        prepare_gantt_family_with_theme_and_engine(
            "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task, 2024-01-01, 1d",
            &theme,
            engine,
        );
    let svg = family::prepare(&fixture_parsed_27, &fixture_layout_27, fixture_session_27)
        .expect("prepare family fixture")
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
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {
                "taskBkgColor": "#111111",
                "activeTaskBkgColor": "#222222",
                "doneTaskBkgColor": "#333333",
                "critBkgColor": "#444444"
            }
        })),
    );
    let (fixture_parsed_28, fixture_layout_28, fixture_session_28) =
        prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
    let artifact = family::prepare(&fixture_parsed_28, &fixture_layout_28, fixture_session_28)
        .expect("prepare family fixture");
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

    assert_eq!(
        terminals.len(),
        8,
        "all eight source-owned task terminals must exist"
    );
    let actual_ids = terminals
        .iter()
        .map(|(id, _, _)| id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected_ids = [
        "gantt-config-default-task",
        "gantt-config-active-task",
        "gantt-config-done-task",
        "gantt-config-crit-task",
        "gantt-config-active-crit-task",
        "gantt-config-done-crit-task",
        "gantt-config-active-milestone-task",
        "gantt-config-done-vert-task",
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual_ids, expected_ids);

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
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({ "themeVariables": { (key): source_fill } }),
            ),
        );
        let (fixture_parsed_29, fixture_layout_29, fixture_session_29) =
            prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
        let artifact = family::prepare(&fixture_parsed_29, &fixture_layout_29, fixture_session_29)
            .expect("prepare family fixture");
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
    let (fixture_parsed_12, fixture_layout_12, fixture_session_12) =
        prepare_gantt_family_with_theme("gantt\ndateFormat YYYY-MM-DD\n", &theme);
    let artifact = family::prepare(&fixture_parsed_12, &fixture_layout_12, fixture_session_12)
        .expect("prepare family fixture");
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
        let (fixture_parsed_13, fixture_layout_13, fixture_session_13) =
            prepare_gantt_family_with_theme(GANTT_TASK_FILL_SOURCE, &theme);
        let artifact = family::prepare(&fixture_parsed_13, &fixture_layout_13, fixture_session_13)
            .expect("prepare family fixture");
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
        let (fixture_parsed_31, fixture_layout_31, fixture_session_31) =
            try_prepare_gantt_family_with_theme_and_engine(
                GANTT_TASK_FILL_SOURCE,
                &theme,
                MermaidEngine::new(),
            );
        let result = family::prepare(&fixture_parsed_31, &fixture_layout_31, fixture_session_31)
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
        let (fixture_parsed_14, fixture_layout_14, fixture_session_14) =
            prepare_gantt_family_with_theme(GANTT_TASK_FILL_SOURCE, &theme);
        let artifact = family::prepare(&fixture_parsed_14, &fixture_layout_14, fixture_session_14)
            .expect("prepare family fixture");
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
        let expected_ids = if variant == ThemeVariant::Error {
            &[
                "gantt-config-crit-task",
                "gantt-config-active-crit-task",
                "gantt-config-done-crit-task",
            ][..]
        } else {
            expected_ids
        };
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
        let (fixture_parsed_32, fixture_layout_32, fixture_session_32) =
            try_prepare_gantt_family_with_theme_and_engine(
                GANTT_TASK_FILL_SOURCE,
                &theme,
                MermaidEngine::new(),
            );
        let result = family::prepare(&fixture_parsed_32, &fixture_layout_32, fixture_session_32)
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
    let (fixture_parsed_15, fixture_layout_15, fixture_session_15) =
        prepare_gantt_family_with_theme("gantt\ndateFormat YYYY-MM-DD\n", &theme);
    let artifact = family::prepare(&fixture_parsed_15, &fixture_layout_15, fixture_session_15)
        .expect("prepare family fixture");
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
        let (fixture_parsed_16, fixture_layout_16, fixture_session_16) =
            prepare_gantt_family_with_theme(source, &theme);
        let error =
            match family::prepare(&fixture_parsed_16, &fixture_layout_16, fixture_session_16)
                .expect("prepare family fixture")
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

        let (fixture_parsed_17, fixture_layout_17, fixture_session_17) =
            prepare_gantt_family_with_theme(&source, &theme);
        let error = family::prepare(&fixture_parsed_17, &fixture_layout_17, fixture_session_17)
            .expect("prepare family fixture")
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
    let (fixture_parsed_18, fixture_layout_18, fixture_session_18) =
        prepare_gantt_family_with_theme(source, &theme);
    let artifact = family::prepare(&fixture_parsed_18, &fixture_layout_18, fixture_session_18)
        .expect("prepare family fixture");
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
    let (fixture_parsed_19, fixture_layout_19, fixture_session_19) =
        prepare_gantt_family_with_theme(source, &theme);
    let artifact = family::prepare(&fixture_parsed_19, &fixture_layout_19, fixture_session_19)
        .expect("prepare family fixture");
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
    render_gantt_svg_from_text_with_engine(MermaidEngine::new(), text)
}

fn render_gantt_svg_from_text_with_engine(engine: MermaidEngine, text: &str) -> String {
    let session = RenderEnvironment::deterministic()
        .with_runtime_policy(
            merman_model::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .begin_session()
        .expect("begin render session");
    let parsed = futures::executor::block_on(
        engine.parse_diagram_for_render_model(text, ParseOptions::default()),
    )
    .expect("parse ok")
    .expect("diagram detected")
    .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
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
            merman_model::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_704_067_200_000),
        )
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Gantt resource-bound session");
    let parsed = futures::executor::block_on(
        MermaidEngine::new().parse_diagram_for_render_model(text, ParseOptions::default()),
    )
    .expect("parse Gantt resource-bound fixture")
    .expect("detect Gantt resource-bound fixture")
    .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
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
fn gantt_viewport_uses_its_own_max_width_setting() {
    for use_max_width in [false, true] {
        let source = format!(
            "---\nconfig:\n  state:\n    useMaxWidth: {}\n  gantt:\n    useMaxWidth: {use_max_width}\n    useWidth: 600.5\n---\ngantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nTask : task, 2026-01-01, 2d\n",
            !use_max_width,
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root = document.root_element();
        let view_box = root
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|number| number.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(view_box[2], 600.5);
        if use_max_width {
            assert_eq!(root.attribute("width"), Some("100%"));
            assert_eq!(root.attribute("height"), None);
            assert_eq!(root.attribute("style"), Some("max-width: 600.5px;"));
        } else {
            for (attribute, expected) in [("width", view_box[2]), ("height", view_box[3])] {
                assert_eq!(
                    root.attribute(attribute).unwrap().parse::<f64>().unwrap(),
                    expected,
                );
            }
            assert_eq!(root.attribute("style"), None);
        }
    }
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
fn gantt_section_titles_share_mermaid_break_rows_and_vertical_alignment() {
    let source = include_str!("../../../fixtures/gantt/multiline_section_title_br_tags.mmd");
    let layout = layout_gantt_from_text(source);
    let section = &layout.section_titles[0];
    let expected = ["Line1", "Line2", "Line3", "Line4", "Line5"];
    assert_eq!(section.lines, expected);
    assert_eq!(section.dy_em, -2.0);

    let svg = render_gantt_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("sectionTitle sectionTitle0"))
        .expect("section title");
    assert_eq!(title.attribute("dy"), Some("-2em"));
    let rows: Vec<_> = title
        .children()
        .filter(|node| node.has_tag_name("tspan"))
        .collect();
    assert_eq!(
        rows.iter()
            .map(|node| node.text().unwrap_or_default())
            .collect::<Vec<_>>(),
        expected
    );
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.attribute("dy"), (index > 0).then_some("1em"));
    }
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
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "securityLevel": "loose"
            })),
        ),
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
fn gantt_svg_tick_interval_uses_config_fallback_and_body_precedence_in_utc() {
    for (body_interval, expected) in [
        ("", &["2024-02-01"][..]),
        (
            "tickInterval 1day\n",
            &[
                "2024-01-30",
                "2024-01-31",
                "2024-02-01",
                "2024-02-02",
                "2024-02-03",
            ][..],
        ),
    ] {
        let source = format!(
            r#"---
config:
  gantt:
    tickInterval: 1month
    topAxis: true
---
gantt
dateFormat YYYY-MM-DD
axisFormat %Y-%m-%d
{body_interval}todayMarker off
section Delivery
Task: task, 2024-01-30, 2024-02-03
"#,
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).expect("valid Gantt SVG XML");
        let axes = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("grid"))
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 2, "both bottom and top axes must render");
        for (index, axis) in axes.iter().enumerate() {
            let labels = axis
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| node.text().expect("axis tick label"))
                .collect::<Vec<_>>();
            assert_eq!(labels, expected, "axis {index}, body: {body_interval:?}");
        }
    }
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
        svg.contains(r#"viewBox="0 0 420 "#) && svg.contains(r#"style="max-width: 420px;""#),
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

#[test]
fn gantt_compound_critical_strokes_have_independent_config_ownership() {
    let theme = gantt_task_stroke_theme(CanvasPaint::solid("#654321").unwrap());
    for critical_owner in [false, true] {
        let variables = if critical_owner {
            serde_json::json!({"critBorderColor":"#444444"})
        } else {
            serde_json::json!({"activeTaskBorderColor":"#222222", "doneTaskBorderColor":"#333333"})
        };
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"themeVariables":variables}),
            ),
        );
        let (fixture_parsed_30, fixture_layout_30, fixture_session_30) =
            prepare_gantt_family_with_theme_and_engine(GANTT_TASK_FILL_SOURCE, &theme, engine);
        let rendered = family::prepare(&fixture_parsed_30, &fixture_layout_30, fixture_session_30)
            .expect("prepare family fixture")
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("gantt-config".into()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .unwrap();
        let terminals = gantt_task_terminal_inline_strokes(rendered.svg());
        assert_eq!(terminals.len(), 8);
        for (id, _, stroke) in terminals {
            let expected = if id.contains("crit-task") {
                if critical_owner { "#444444" } else { "#654321" }
            } else if !critical_owner
                && (id.contains("active-task") || id.contains("active-milestone"))
            {
                "#222222"
            } else if !critical_owner && (id.contains("done-task") || id.contains("done-vert")) {
                "#333333"
            } else {
                "#654321"
            };
            assert_eq!(
                stroke, expected,
                "critical owner={critical_owner}, terminal {id}"
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn gantt_active_done_labels_preserve_upstream_classes_and_outside_paint_ownership() {
    let theme = gantt_text_fill_theme(CanvasPaint::solid("#abcdef").unwrap(), None);
    for critical in [false, true] {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD\nsection Work\nAn extremely long outside task label: active, done, {}task, 2024-01-01, 1d\nLong span: span, 2024-01-01, 100d\n",
            if critical { "crit, " } else { "" }
        );
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": {"taskTextOutsideColor": "#123456"}
            })),
        );
        for themed in [false, true] {
            let svg = if themed {
                let (parsed, options, session) =
                    prepare_gantt_family_with_theme_and_engine(&source, &theme, engine.clone());
                family::prepare(&parsed, &options, session)
                    .unwrap()
                    .render_svg(
                        &SvgRenderOptions {
                            diagram_id: Some("gantt-config".into()),
                            ..Default::default()
                        },
                        &SvgDebugOptions::default(),
                    )
                    .unwrap()
                    .svg()
                    .to_owned()
            } else {
                render_gantt_svg_from_text_with_engine(engine.clone(), &source)
            };
            let document = roxmltree::Document::parse(&svg).unwrap();
            let label = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("text")
                        && node
                            .attribute("id")
                            .is_some_and(|id| id.ends_with("-task-text"))
                })
                .unwrap();
            let classes = label.attribute("class").unwrap();
            let active = if critical {
                "activeCritText0"
            } else {
                "activeText0"
            };
            let done = if critical {
                "doneCritText0"
            } else {
                "doneText0"
            };
            assert!(
                classes
                    .split_ascii_whitespace()
                    .any(|class| class == active),
                "{classes}"
            );
            assert!(
                classes.split_ascii_whitespace().any(|class| class == done),
                "{classes}"
            );
            assert!(classes.contains("taskTextOutside"), "{classes}");
            assert!(
                label.attribute("style").is_none(),
                "source-owned outside paint must retain its CSS winner"
            );
            assert!(svg.contains("fill:#123456;"), "{svg}");
        }
    }
}

#[test]
fn gantt_compact_sections_use_javascript_object_key_order_without_reordering_semantics() {
    let source = concat!(
        "---\nconfig:\n  gantt:\n    displayMode: compact\n---\n",
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\n",
        "section 2\nA: a,2026-01-01,1d\n",
        "section 1\nB: b,2026-01-01,1d\n",
        "section 01\nC: c,2026-01-01,1d\n",
        "section Later\nD: d,2026-01-01,1d\n",
    );
    let layout = layout_gantt_from_text(source);
    assert_eq!(
        layout
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c", "d"],
    );
    assert_eq!(
        layout
            .tasks
            .iter()
            .map(|task| task.order)
            .collect::<Vec<_>>(),
        [1, 0, 2, 3],
    );
    assert_eq!(
        layout
            .section_titles
            .iter()
            .map(|title| title.section.as_str())
            .collect::<Vec<_>>(),
        ["1", "2", "01", "Later"],
    );
    let svg = render_gantt_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let titles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.starts_with("sectionTitle "))
        })
        .map(|node| {
            node.descendants()
                .filter_map(|child| child.text())
                .next()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(titles, ["1", "2", "01", "Later"]);
}

#[test]
fn gantt_duplicate_id_labels_keep_their_semantic_occurrence_after_date_sorting() {
    let source = concat!(
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Work\n",
        "Later: done,dup,2026-01-02,1d\n",
        "First: active,dup,2026-01-01,1d\n",
    );
    let svg = render_gantt_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (text, expected, other) in [
        ("First", "activeText0", "doneText0"),
        ("Later", "doneText0", "activeText0"),
    ] {
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("text") && node.text() == Some(text))
            .unwrap();
        let classes = label.attribute("class").unwrap();
        assert!(
            classes
                .split_ascii_whitespace()
                .any(|class| class == expected),
            "{classes}"
        );
        assert!(
            !classes.split_ascii_whitespace().any(|class| class == other),
            "{classes}"
        );
    }
}

#[test]
fn gantt_custom_axis_retains_all_admitted_ticks_and_falls_back_above_the_upstream_limit() {
    for (end, expected_count) in [("2026-04-11", Some(2401)), ("2027-03-01", None)] {
        let source = format!(
            concat!(
                "gantt\ndateFormat YYYY-MM-DD\naxisFormat %Y-%m-%d %H:%M\n",
                "tickInterval 1hour\ntodayMarker off\nTask: task,2026-01-01,{end}\n",
            ),
            end = end
        );
        let layout = layout_gantt_from_text(&source);
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let ticks = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("tick"))
            .count();
        assert_eq!(ticks, layout.bottom_ticks.len());
        if let Some(count) = expected_count {
            assert_eq!(ticks, count);
            assert_eq!(
                layout.bottom_ticks.first().unwrap().label,
                "2026-01-01 00:00"
            );
            assert_eq!(
                layout.bottom_ticks.last().unwrap().label,
                "2026-04-11 00:00"
            );
        } else {
            assert!(
                ticks < 100,
                "excessive custom intervals must use the automatic scale"
            );
        }
    }
}

#[test]
fn gantt_excluded_days_remain_visible_until_six_calendar_years() {
    for (end, count) in [("2025-07-01", 287), ("2026-01-01", 0)] {
        let source = format!(
            concat!(
                "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nexcludes weekends\n",
                "Task: task,2020-01-01,{end}\n",
            ),
            end = end
        );
        let layout = layout_gantt_from_text(&source);
        assert_eq!(layout.excludes.len(), count);
        assert_eq!(layout.has_excludes_layer, count > 0);
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect") && node.attribute("class") == Some("exclude-range")
                })
                .count(),
            count
        );
    }
}

#[test]
fn gantt_exclusion_matching_keeps_the_scan_clock_and_floors_only_rectangle_geometry() {
    use merman_model::time::{CivilDate, OffsetDateTime, UtcOffset};

    let ms = |day, hour| {
        OffsetDateTime::from_local(
            CivilDate::new(2024, 1, day)
                .unwrap()
                .at_hms_milli(hour, 0, 0, 0)
                .unwrap(),
            UtcOffset::UTC,
        )
        .unwrap()
        .timestamp_millis()
    };
    // ganttRenderer.drawExcludeDays keeps dayjs(minTime)'s clock until SVG geometry is derived.
    for (exclusion, includes, shaded) in [
        ("2024-01-02_12:00", "", true),
        ("2024-01-02", "", true),
        ("2024-01-02_00:00", "", false),
        ("2024-01-02_12:00", "includes 2024-01-02_12:00\n", false),
        ("2024-01-02_12:00", "includes 2024-01-02_00:00\n", true),
    ] {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD_HH:mm\nexcludes {exclusion}\n{includes}todayMarker off\nTask: task,2024-01-01_12:00,2d\n"
        );
        let layout = layout_gantt_from_text(&source);
        assert_eq!(layout.tasks[0].start_ms, ms(1, 12));
        assert_eq!(layout.tasks[0].end_ms, ms(if shaded { 4 } else { 3 }, 12));
        assert_eq!(
            layout.excludes.len(),
            usize::from(shaded),
            "{exclusion}, {includes}"
        );

        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let rectangles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("rect") && node.attribute("class") == Some("exclude-range")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rectangles.len(),
            usize::from(shaded),
            "{exclusion}, {includes}"
        );
        if !shaded {
            continue;
        }
        let excluded = &layout.excludes[0];
        assert_eq!(excluded.id, "exclude-2024-01-02");
        assert_eq!(excluded.start_ms, ms(2, 0));
        assert_eq!(excluded.end_ms, ms(3, 0) - 1);
        let rectangle = rectangles[0];
        assert_eq!(
            rectangle.attribute("id"),
            Some("gantt-config-exclude-2024-01-02")
        );
        for (attribute, expected) in [("x", excluded.x), ("width", excluded.width)] {
            assert_eq!(
                rectangle
                    .attribute(attribute)
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                expected
            );
        }
        let range = layout.width - layout.left_padding - layout.right_padding;
        let clock_x = layout.left_padding + (range / 3.0).round();
        let expected_origin = format!("{clock_x}px");
        assert_eq!(
            rectangle
                .attribute("transform-origin")
                .unwrap()
                .split_ascii_whitespace()
                .next(),
            Some(expected_origin.as_str()),
            "transform origin retains noon while the rectangle begins at midnight"
        );
    }
}

#[test]
fn gantt_field_filtered_axes_reset_fields_across_minutes_days_and_years() {
    // Mermaid 12.3.0 calls d3-time 3.1.1's every(field % step), including steps larger than a cycle.
    for (date_format, axis_format, interval, start, duration, expected) in [
        (
            "YYYY-MM-DD_HH:mm",
            "%Y-%m-%d_%H:%M",
            "5hour",
            "2024-01-01_20:00",
            "10h",
            &["2024-01-01_20:00", "2024-01-02_00:00", "2024-01-02_05:00"][..],
        ),
        (
            "YYYY-MM-DD_HH:mm",
            "%Y-%m-%d_%H:%M",
            "4hour",
            "2024-01-01_20:00",
            "10h",
            &["2024-01-01_20:00", "2024-01-02_00:00", "2024-01-02_04:00"][..],
        ),
        (
            "YYYY-MM-DD_HH:mm",
            "%Y-%m-%d_%H:%M",
            "25hour",
            "2024-01-01_20:00",
            "72h",
            &["2024-01-02_00:00", "2024-01-03_00:00", "2024-01-04_00:00"][..],
        ),
        (
            "YYYY-MM-DD_HH:mm:ss",
            "%Y-%m-%d_%H:%M:%S",
            "7second",
            "2024-01-01_00:00:56",
            "10s",
            &["2024-01-01_00:00:56", "2024-01-01_00:01:00"][..],
        ),
        (
            "YYYY-MM-DD_HH:mm:ss",
            "%Y-%m-%d_%H:%M:%S",
            "5second",
            "2024-01-01_00:00:56",
            "10s",
            &["2024-01-01_00:01:00", "2024-01-01_00:01:05"][..],
        ),
        (
            "YYYY-MM-DD",
            "%Y-%m-%d",
            "5month",
            "2024-11-01",
            "160d",
            &["2024-11-01", "2025-01-01"][..],
        ),
        (
            "YYYY-MM-DD",
            "%Y-%m-%d",
            "3month",
            "2024-11-01",
            "160d",
            &["2025-01-01", "2025-04-01"][..],
        ),
        (
            "YYYY-MM-DD",
            "%Y-%m-%d",
            "13month",
            "2024-11-01",
            "800d",
            &["2025-01-01", "2026-01-01", "2027-01-01"][..],
        ),
    ] {
        let source = format!(
            "gantt\ndateFormat {date_format}\naxisFormat {axis_format}\ntickInterval {interval}\ntodayMarker off\nTask: task,{start},{duration}\n"
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let axes = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("grid"))
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 1, "{interval}");
        let labels = axes[0]
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(labels, expected, "{interval}");
    }
}

#[test]
fn gantt_filtered_minute_config_and_body_precedence_reach_both_axes() {
    for (configured_interval, body_interval, expected) in [
        ("7minute", "", &["2024-01-01_00:56", "2024-01-01_01:00"][..]),
        ("5minute", "", &["2024-01-01_01:00", "2024-01-01_01:05"][..]),
        (
            "5minute",
            "tickInterval 7minute\n",
            &["2024-01-01_00:56", "2024-01-01_01:00"][..],
        ),
    ] {
        let source = format!(
            "---\nconfig:\n  gantt:\n    tickInterval: {configured_interval}\n    topAxis: true\n---\ngantt\ndateFormat YYYY-MM-DD_HH:mm\naxisFormat %Y-%m-%d_%H:%M\n{body_interval}todayMarker off\nTask: task,2024-01-01_00:56,10m\n"
        );
        let svg = render_gantt_svg_from_text(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let axes = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("grid"))
            .collect::<Vec<_>>();
        assert_eq!(axes.len(), 2);
        for axis in axes {
            let labels = axis
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| node.text().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                labels, expected,
                "config {configured_interval}, body {body_interval:?}"
            );
        }
    }
}

#[allow(dead_code)]
mod common;
